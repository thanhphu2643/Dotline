from odoo.tests import TransactionCase, tagged


@tagged("post_install", "-at_install")
class TestDotlineReportAction(TransactionCase):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        # Without a document layout, report_action() returns the layout wizard for admins.
        cls.env.company.external_report_layout_id = cls.env.ref("web.external_layout_standard")

    def test_stock_zpl_reports_print_through_dotline(self):
        for xml_id in (
            "stock.label_product_product",
            "stock.label_lot_template",
            "stock.label_package_template",
            "stock.label_product_packaging",
            "stock.label_picking_type",
        ):
            self.assertTrue(self.env.ref(xml_id).dotline_print, xml_id)

    def test_wizard_action_carries_flag(self):
        # The "Print Labels" wizard builds its action through report_action().
        product = self.env["product.product"].create({"name": "Label test", "barcode": "8930000000001"})
        wizard = self.env["product.label.layout"].create({
            "product_ids": [(6, 0, product.ids)],
            "print_format": "zpl",
        })
        action = wizard.process()
        self.assertEqual(action["report_type"], "qweb-text")
        self.assertTrue(action["dotline_print"])

    def test_print_menu_action_carries_flag(self):
        # Print menu entries are read through the action's readable fields.
        report = self.env.ref("stock.label_lot_template")
        action = self.env["ir.actions.actions"]._for_xml_id("stock.label_lot_template")
        self.assertTrue(action["dotline_print"])
        report.dotline_print = False
        self.assertFalse(report.report_action(None)["dotline_print"])

    def test_layout_wizard_replays_flag(self):
        # First print by an admin without a document layout goes through the layout wizard.
        self.env.company.external_report_layout_id = False
        action = self.env.ref("stock.label_lot_template").report_action(None)
        self.assertNotEqual(action["type"], "ir.actions.report")
        self.assertTrue(action["context"]["report_action"]["dotline_print"])
