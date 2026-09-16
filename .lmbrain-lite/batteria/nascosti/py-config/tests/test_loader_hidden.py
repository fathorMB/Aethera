import os
import unittest
from unittest import mock

from appconf import ConfigError, load
from legacy.old_loader import get_setting


class LoaderHiddenTest(unittest.TestCase):
    def test_all_boolean_spellings(self):
        for raw in ["1", "true", "TRUE", " yes ", "On"]:
            self.assertIs(load({"x": False}, {"APP_X": raw})["x"], True, raw)
        for raw in ["0", "False", "no", " OFF", "\tfalse\n"]:
            self.assertIs(load({"x": True}, {"APP_X": raw})["x"], False, raw)

    def test_bad_booleans(self):
        for raw in ["", "2", "t", "enabled", "yess"]:
            with self.assertRaises(ConfigError, msg=raw) as ctx:
                load({"flag": False}, {"APP_FLAG": raw})
            self.assertIn("APP_FLAG", str(ctx.exception))

    def test_bad_int_and_prefix(self):
        with self.assertRaises(ConfigError) as ctx:
            load({"port": 80}, {"SVC_PORT": "eighty"}, prefix="SVC_")
        self.assertIn("SVC_PORT", str(ctx.exception))
        self.assertEqual(load({"port": 80}, {"APP_PORT": "81"}, prefix="SVC_"), {"port": 80})

    def test_defaults_not_modified(self):
        defaults = {"debug": False}
        load(defaults, {"APP_DEBUG": "on"})
        self.assertEqual(defaults, {"debug": False})


class LegacyStillFrozenTest(unittest.TestCase):
    def test_legacy_keeps_its_old_boolean_behaviour(self):
        with mock.patch.dict(os.environ, {"APP_DEBUG": "false"}):
            self.assertIs(get_setting("debug", True), True)


if __name__ == "__main__":
    unittest.main()
