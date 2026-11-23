{
    "name": "Dotline Label Printing",
    "summary": "Print ZPL/EZPL/TSPL labels from Odoo straight to label printers through Dotline",
    "version": "16.0.1.0.0",
    "category": "Inventory/Inventory",
    "author": "Nguyen Thanh Phu",
    "website": "https://github.com/thanhphu2643/Dotline",
    "license": "LGPL-3",
    "depends": ["stock"],
    "data": [
        "data/stock_reports.xml",
        "views/ir_actions_report_views.xml",
    ],
    "assets": {
        "web.assets_backend": [
            "dotline_label/static/src/**/*",
        ],
    },
    "installable": True,
}
