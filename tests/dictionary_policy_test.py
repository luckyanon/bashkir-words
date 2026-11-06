import copy
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / 'scripts'))
from dictionary_policy import apply_policy, is_abbreviation


class DictionaryPolicyTest(unittest.TestCase):
    def test_import_removes_abbreviations_and_restores_qaysy_idempotently(self):
        entries = [{'display': word, 'definitions': {'ru': [{'text': text}]}}
                   for word, text in [('КГ', 'килограмм'), ('МӘК', 'мак'), ('ГЕР', 'гиря'),
                                      ('АЙ', 'луна'), ('ҠЫҪҠАРТМА', 'сокращение'),
                                      ('ИҢЕШ', 'сокращение длины продольных ниток')]]
        raw, full = {'starterWords': ['АЙ', 'КГ']}, {'entries': entries}
        apply_policy(raw, full)
        self.assertEqual(set(raw['words']), {'АЙ', 'ҠЫҪҠАРТМА', 'ИҢЕШ', 'ҠАЙСЫ'})
        self.assertEqual(raw['starterWords'], ['АЙ'])
        before = copy.deepcopy((raw, full))
        apply_policy(raw, full)
        self.assertEqual((raw, full), before)

    def test_new_source_marked_abbreviations_are_rejected(self):
        for sense in [{'text': 'единица (сокращение)'}, {'text': 'unit', 'tags': ['abbreviation']},
                      {'text': 'short for a longer name'}, {'text': 'сокр. от названия'}]:
            self.assertTrue(is_abbreviation({'display': 'ТЕСТ', 'definitions': {'ru': [sense]}}))


if __name__ == '__main__':
    unittest.main()
