import unittest

from appconf import ConfigError, load

DEFAULTS = {"debug": False, "workers": 4, "ratio": 0.5, "name": "svc"}


class LoaderTest(unittest.TestCase):
    def test_defaults_only(self):
        self.assertEqual(load(DEFAULTS, {}), DEFAULTS)

    def test_overrides(self):
        env = {"APP_WORKERS": " 8 ", "APP_NAME": "api", "APP_RATIO": "0.25"}
        self.assertEqual(load(DEFAULTS, env), {"debug": False, "workers": 8, "ratio": 0.25, "name": "api"})

    def test_false_strings_are_false(self):
        self.assertIs(load({"debug": True}, {"APP_DEBUG": "false"})["debug"], False)
        self.assertIs(load({"debug": True}, {"APP_DEBUG": "0"})["debug"], False)

    def test_true_strings_are_true(self):
        self.assertIs(load(DEFAULTS, {"APP_DEBUG": "Yes"})["debug"], True)

    def test_bad_boolean_names_the_variable(self):
        with self.assertRaises(ConfigError) as ctx:
            load(DEFAULTS, {"APP_DEBUG": "maybe"})
        self.assertIn("APP_DEBUG", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
