import unittest

from scripts.release_notes import dedupe_issues, extract_issue_numbers, normalize_version, render_issue_section


class ReleaseNotesHelpersTest(unittest.TestCase):
    def test_normalize_version_strips_v_prefix(self):
        self.assertEqual(normalize_version("v0.0.3"), "0.0.3")
        self.assertEqual(normalize_version("0.0.3"), "0.0.3")

    def test_extract_issue_numbers_dedupes_and_sorts(self):
        text = "Closes #12\nRefs #3\nAlso relates to #12"
        self.assertEqual(extract_issue_numbers(text), [3, 12])

    def test_dedupe_issues_keeps_latest_copy_by_number(self):
        items = [
            {"number": 2, "title": "旧标题"},
            {"number": 1, "title": "第一个"},
            {"number": 2, "title": "新标题"},
        ]
        self.assertEqual(
            dedupe_issues(items),
            [
                {"number": 1, "title": "第一个"},
                {"number": 2, "title": "新标题"},
            ],
        )

    def test_render_issue_section_includes_milestone_when_present(self):
        section = render_issue_section(
            [
                {
                    "number": 2,
                    "title": "规范 issue 默认指派与 release issue 归集机制",
                    "html_url": "https://github.com/fanbuz/codex-threads/issues/2",
                }
            ],
            "0.0.3",
        )
        self.assertIn("## 关联 Issue", section)
        self.assertIn("里程碑：`0.0.3`", section)
        self.assertIn("#2 规范 issue 默认指派与 release issue 归集机制", section)


if __name__ == "__main__":
    unittest.main()
