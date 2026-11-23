/** @odoo-module **/

import { Component, useState, xml } from "@odoo/owl";
import { Dialog } from "@web/core/dialog/dialog";
import { registry } from "@web/core/registry";

const DOTLINE_URL = "http://127.0.0.1:17820";
const STORAGE_KEY = "dotline_label.config";
const DOWNLOAD_URL = "https://github.com/thanhphu2643/Dotline/releases/latest";

// Each computer runs its own Dotline with its own token, so the settings belong
// to the browser, not to the Odoo user.
function loadConfig() {
    try {
        return JSON.parse(localStorage.getItem(STORAGE_KEY)) || {};
    } catch {
        return {};
    }
}

class DotlineError extends Error {
    constructor(code) {
        super(code);
        this.code = code;
    }
}

// Throws TypeError when Dotline is not running (the request never reaches it).
async function dotline(path, token, init = {}) {
    const response = await fetch(DOTLINE_URL + path, {
        ...init,
        headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    });
    const body = await response.json().catch(() => ({}));
    if (!response.ok && !body.state) {
        throw new DotlineError(body.error || `http${response.status}`);
    }
    return body;
}

class DotlineSetupDialog extends Component {
    static template = xml`
        <Dialog title="env._t('Dotline label printer')" size="'md'">
            <p>
                Copy the token from the Dotline app on this computer, then pick the label printer.
                These settings are kept in this browser only.
            </p>
            <div class="mb-3">
                <label class="form-label" for="dotline_token">Token</label>
                <input id="dotline_token" type="password" autocomplete="off" class="form-control" t-model.trim="state.token" t-on-change="loadPrinters"/>
            </div>
            <div class="mb-3">
                <label class="form-label" for="dotline_printer">Printer</label>
                <input id="dotline_printer" class="form-control" list="dotline_printers" t-model.trim="state.printer"/>
                <datalist id="dotline_printers">
                    <option t-foreach="state.printers" t-as="printer" t-key="printer.name" t-att-value="printer.name"/>
                </datalist>
                <div class="form-text">Pick a printer from Dotline, or type a network printer as tcp://192.168.1.50:9100.</div>
            </div>
            <div t-if="state.error" class="alert alert-warning mb-0" role="alert" t-esc="state.error"/>
            <t t-set-slot="footer">
                <button class="btn btn-primary" t-on-click="save" t-att-disabled="!state.token or !state.printer">Save and print</button>
                <button class="btn btn-secondary" t-on-click="props.close">Cancel</button>
            </t>
        </Dialog>`;
    static components = { Dialog };

    setup() {
        const { token = "", printer = "" } = loadConfig();
        this.state = useState({ token, printer, printers: [], error: this.props.reason || "" });
        if (token) {
            this.loadPrinters();
        }
    }

    async loadPrinters() {
        this.state.printers = [];
        if (!this.state.token) {
            return;
        }
        try {
            const { printers } = await dotline("/printers", this.state.token);
            this.state.printers = printers;
            this.state.error = "";
            if (!this.state.printer) {
                this.state.printer = (printers.find((printer) => printer.isDefault) || printers[0] || {}).name || "";
            }
        } catch (error) {
            this.state.error = describeError(this.env, error);
        }
    }

    save() {
        const config = { token: this.state.token, printer: this.state.printer };
        localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
        this.props.onSave(config);
        this.props.close();
    }
}

function describeError(env, error) {
    switch (error.code) {
        case undefined:
            return env._t("Dotline is not running on this computer.");
        case "unauthorized":
            return env._t("Dotline rejected the token. Copy it again from the Dotline app.");
        case "printerNotFound":
            return env._t("This printer is not available in Dotline any more.");
        case "printersUnavailable":
            return env._t("Dotline cannot list the printers of this computer.");
        default:
            return env._t("Dotline returned an error: ") + error.code;
    }
}

function askForConfig(env, reason) {
    return new Promise((resolve) => {
        env.services.dialog.add(
            DotlineSetupDialog,
            { reason, onSave: resolve },
            { onClose: () => resolve(null) }
        );
    });
}

// Same URL the web client builds for a text report download (action_service.js, _getReportUrl).
function reportUrl(action, env) {
    let url = `/report/text/${action.report_name}`;
    const context = action.context || {};
    if (action.data && JSON.stringify(action.data) !== "{}") {
        url += `?options=${encodeURIComponent(JSON.stringify(action.data))}`;
        url += `&context=${encodeURIComponent(JSON.stringify(context))}`;
    } else {
        if (context.active_ids) {
            url += `/${context.active_ids.join(",")}`;
        }
        url += `?context=${encodeURIComponent(JSON.stringify(env.services.user.context))}`;
    }
    return url;
}

// Base64 of the exact bytes Odoo rendered, without assuming the text is ASCII.
async function toBase64(blob) {
    const dataUrl = await new Promise((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(reader.result);
        reader.onerror = () => reject(reader.error);
        reader.readAsDataURL(blob);
    });
    return dataUrl.slice(dataUrl.indexOf(",") + 1);
}

async function sendToDotline(action, env, config) {
    const response = await fetch(reportUrl(action, env));
    if (!response.ok) {
        throw new Error(env._t("Odoo could not render the label."));
    }
    const job = await dotline("/jobs", config.token, {
        method: "POST",
        body: JSON.stringify({
            id: `odoo-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`,
            printer: config.printer,
            payloadBase64: await toBase64(await response.blob()),
            title: action.name,
        }),
    });
    if (job.state === "failed") {
        throw new DotlineError(job.detail || "failed");
    }
    return job;
}

async function dotlineReportHandler(action, options, env) {
    if (action.report_type !== "qweb-text" || !action.dotline_print) {
        return false;
    }
    const notification = env.services.notification;
    let config = loadConfig();
    let reason;
    // At most one retry through the setup dialog (missing config, bad token, printer gone).
    for (let attempt = 0; attempt < 2; attempt++) {
        if (!config.token || !config.printer) {
            config = await askForConfig(env, reason);
            if (!config) {
                return true; // the user cancelled: print nothing
            }
        }
        env.services.ui.block();
        try {
            await sendToDotline(action, env, config);
            notification.add(env._t("Sent to %s").replace("%s", config.printer), {
                type: "success",
                buttons: [{ name: env._t("Change printer"), onClick: () => askForConfig(env) }],
            });
            break;
        } catch (error) {
            if (error instanceof TypeError) {
                // Dotline is not running: keep Odoo's normal behaviour and download the file.
                notification.add(env._t("Dotline is not running on this computer, so the label file was downloaded instead."), {
                    type: "warning",
                    buttons: [{ name: env._t("Get Dotline"), onClick: () => window.open(DOWNLOAD_URL, "_blank") }],
                });
                return false;
            }
            if (["unauthorized", "printerNotFound"].includes(error.code) && attempt === 0) {
                reason = describeError(env, error);
                config = {};
                continue;
            }
            notification.add(error.code ? describeError(env, error) : error.message, { type: "danger" });
            break;
        } finally {
            env.services.ui.unblock();
        }
    }
    if (action.close_on_report_download) {
        await env.services.action.doAction({ type: "ir.actions.act_window_close" }, { onClose: options.onClose });
    } else if (options.onClose) {
        options.onClose();
    }
    return true;
}

registry.category("ir.actions.report handlers").add("dotline_label", dotlineReportHandler);
