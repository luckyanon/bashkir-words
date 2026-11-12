import importlib.util
import pathlib
import unittest

spec = importlib.util.spec_from_file_location('audit', pathlib.Path(__file__).resolve().parents[1] / 'scripts/audit-dictionary.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class DictionaryAuditTest(unittest.TestCase):
    def test_counts_only_order_preserving_insertions_and_deletions(self):
        self.assertEqual(audit.insertion_difference('АУЫЛ', 'аул')['extraLetters'], 'ы')
        self.assertEqual(audit.insertion_difference('манжур', 'маньчжур')['difference'], 2)
        self.assertEqual(audit.insertion_difference('фрикатив', 'фрикативный')['difference'], 3)
        self.assertIsNone(audit.insertion_difference('пила', 'липа'))
        self.assertIsNone(audit.insertion_difference('кот', 'рот'))
        self.assertIsNone(audit.insertion_difference('тест', 'тест'))

    def test_distinct_letters_are_never_folded_into_false_matches(self):
        self.assertIsNone(audit.insertion_difference('елка', 'ёлочка'))
        self.assertIsNone(audit.insertion_difference('ҡалын', 'калым'))
        self.assertIsNone(audit.insertion_difference('aл', 'аллах'))
        self.assertEqual(audit.insertion_difference('ЕЛКА', 'елкаа')['difference'], 1)

    def test_alternatives_handle_labels_nested_parentheses_and_punctuation(self):
        self.assertEqual(list(audit.alternatives('уст., ботан. ёлка (растение (дерево)); ель или сосна/кедр')),
                         ['ёлка', 'ель', 'сосна', 'кедр'])
        self.assertEqual(list(audit.alternatives('большой дом, матем. ряд; ряд домов')),
                         ['ряд'])
        self.assertEqual(list(audit.alternatives('порошок; PLpowder')), ['порошок'])

    def test_length_rule_applies_to_bashkir_word_and_pairs_are_deduplicated(self):
        def entry(word, text):
            return {'word': word, 'display': word.upper(), 'definitions': {'ru': [{'text': text}, {'text': text}]}}
        result = audit.pairs([entry('абвгд', 'абвгдеж'), entry('абвгде', 'абвг'), entry('ал', 'аллах'), entry('секунд', 'секунда')])
        self.assertEqual([(p['word'], p['translation']) for p in result], [('СЕКУНД', 'секунда'), ('АБВГДЕ', 'абвг')])
        self.assertFalse(result[1]['bothAtLeastSix'])


if __name__ == '__main__':
    unittest.main()
