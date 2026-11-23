from odoo import fields, models


class IrActionsReport(models.Model):
    _inherit = "ir.actions.report"

    dotline_print = fields.Boolean(
        string="Print via Dotline",
        help="Send the rendered text straight to a label printer through the Dotline app "
        "running on the user's computer, instead of downloading a file. "
        "Falls back to the download when Dotline is not running.",
    )

    def _get_readable_fields(self):
        # Reports opened from a Print menu are read through these fields.
        return super()._get_readable_fields() | {"dotline_print"}

    def report_action(self, docids, data=None, config=True):
        # Reports opened from wizards (e.g. "Print Labels") are built here instead.
        action = super().report_action(docids, data=data, config=config)
        if action.get("type") == "ir.actions.report":
            action["dotline_print"] = self.dotline_print
        return action

    def _action_configure_external_report_layout(self, report_action):
        # Admins without a document layout get the layout wizard first, which then
        # replays this action: it must keep the flag too.
        report_action["dotline_print"] = self.dotline_print
        return super()._action_configure_external_report_layout(report_action)
