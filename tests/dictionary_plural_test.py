import copy
import importlib.util
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('plural_audit', ROOT/'scripts/audit-plurals.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)
sys.path.insert(0, str(ROOT/'scripts'))
from dictionary_policy import apply_policy


class PluralAuditRulesTest(unittest.TestCase):
    def test_all_eight_suffixes_and_consonant_assimilation(self):
        examples = {'бала': 'лар', 'кәмә': 'ләр', 'китап': 'тар', 'эш': 'тәр',
                    'урман': 'дар', 'күл': 'дәр', 'ҡыҙ': 'ҙар', 'ир': 'ҙәр'}
        for stem, suffix in examples.items():
            with self.subTest(stem=stem):
                self.assertEqual(audit.expected_suffix(stem), suffix)
        self.assertEqual(audit.expected_suffix('ҡан'), 'дар')
        self.assertFalse(audit.suffix_analysis('ҠАНТАР', {'ҠАН'})['spellingCompatible'])

    def test_suffix_coincidence_is_not_a_reviewed_plural(self):
        for word in ['АЙҘАР', 'АЛДАР', 'АҪТАР', 'ДАРДАР', 'ДӘФТӘР', 'ХӨКӨМДАР']:
            self.assertIsNotNone(audit.suffix_analysis(word, set()))
            self.assertNotIn(word, audit.REVIEWED)
        self.assertIsNone(audit.suffix_analysis('ҠЫҘЫ', set()))

    def test_long_description_is_not_split_into_a_fake_translation(self):
        self.assertEqual(list(audit.russian_segments('головной убор, украшенный длинными подвесками и бусами')), [])
        self.assertEqual(list(audit.russian_segments('ботан. листва; листья, ветки')), ['листва', 'листья', 'ветки'])

    def test_import_excludes_requested_form_only_and_is_idempotent(self):
        preserved = ['ҠЫҘ', 'УЛЫ', 'ҠАЙСЫ', 'ЙӘШТӘР', 'БАҪМАҠТАР', 'ҺӨТИМӘРҘӘР']
        entries = [{'display': word, 'definitions': {'ru': [{'text': 'исходный перевод'}]}}
                   for word in ['ҠЫҘЫ'] + preserved]
        raw, full = {'starterWords': ['ҠЫҘЫ', 'ҠЫҘ']}, {'entries': copy.deepcopy(entries)}
        apply_policy(raw, full)
        self.assertEqual(set(raw['words']), set(preserved))
        self.assertEqual(full['entries'], entries[1:])
        self.assertEqual(raw['starterWords'], ['ҠЫҘ'])
        before = copy.deepcopy((raw, full))
        apply_policy(raw, full)
        self.assertEqual((raw, full), before)


@unittest.skipUnless(importlib.util.find_spec('pymorphy3'), 'Install scripts/requirements-dictionary-audit.txt for Russian morphology tests')
class RussianNumberAuditTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.analyser = audit.RussianNumberAnalyser()

    def test_daughter_ambiguity_and_pluralia_tantum_are_separate(self):
        self.assertEqual(self.analyser.analyse('дочери')['category'], 'ambiguous_number')
        self.assertTrue(self.analyser.analyse('дочери')['singularAlsoPossible'])
        self.assertEqual(self.analyser.analyse('ножницы')['category'], 'russian_pluralia_tantum')
        self.assertIsNone(self.analyser.analyse('дочь'))

    def test_oblique_homographs_do_not_enter_headword_plural_list(self):
        for text in ['бекасам', 'манжет', 'белок', 'приспособлениях', 'подвесками']:
            with self.subTest(text=text):
                self.assertIsNone(self.analyser.analyse(text))
        self.assertEqual(self.analyser.analyse('короткие поводья')['category'], 'russian_plural')

    def test_russian_number_does_not_make_bashkir_lemma_plural(self):
        full = {'datasetRevision': 'fixture', 'entries': [
            {'display': 'ҠАЙСЫ', 'definitions': {'ru': [{'text': 'ножницы'}]}},
            {'display': 'БОЛОТ', 'definitions': {'ru': [{'text': 'облако'}]}},
            {'display': 'АЛДАР', 'definitions': {'ru': [{'text': 'обманщик'}]}},
        ]}
        before = copy.deepcopy(full)
        report = audit.audit(full, self.analyser)
        self.assertEqual(full, before)
        self.assertEqual(report['bashkirPluralCandidates'], [])
        self.assertEqual([r['word'] for r in report['russianTranslationCandidates']], ['ҠАЙСЫ'])
        self.assertFalse(report['russianTranslationCandidates'][0]['bashkirPluralIndependentlyReviewed'])


if __name__ == '__main__':
    unittest.main()
