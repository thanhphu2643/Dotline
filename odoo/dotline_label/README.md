# Dotline Label Printing for Odoo

Print Odoo's ZPL labels straight to a label printer, without downloading a file
and sending it by hand. Works with Odoo 16.

Odoo already renders labels as ZPL (product, lot/serial, package, product packaging
and operation type labels, plus the **Print Labels** wizard with the *ZPL Labels*
format). This module intercepts those reports in the browser and hands the bytes to
[Dotline](../../README.md) running on the same computer, which sends them to a CUPS
queue or a network printer (`tcp://host:9100`).

## How it works

```
Print Labels → Odoo renders ZPL → browser posts it to Dotline (127.0.0.1:17820) → printer
```

The browser talks to Dotline, not the Odoo server, so it works the same with Odoo on
your own server or in the cloud.

- **First print:** a dialog asks for the Dotline token (copy it from the Dotline app)
  and the printer. Pick one of the printers Dotline lists, or type a network printer
  as `tcp://192.168.1.50:9100`. Both are kept in this browser, because each computer
  runs its own Dotline.
- **Wrong token or printer gone:** the dialog opens again and says why.
- **Dotline not running:** Odoo downloads the label file as usual and shows a link to
  install Dotline.

Any other text report can print through Dotline too: open it in
*Settings → Technical → Reports* and tick **Print via Dotline** (shown for text reports).

## Install

Copy `dotline_label` into your addons path, update the apps list and install
**Dotline Label Printing**. It depends on `stock`.

## Tests

```sh
odoo -d <db> -i dotline_label --test-enable --test-tags /dotline_label --stop-after-init
```
