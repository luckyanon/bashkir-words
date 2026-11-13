"""Read-only number/form audit. Generates reports; never modifies the dictionary.

Install scripts/requirements-dictionary-audit.txt, then run from the repo:
    python scripts/audit-plurals.py
Russian morphology is evidence about the translation, not the Bashkir lemma.
"""
import argparse
from collections import Counter
from functools import lru_cache
import importlib.metadata
import json
from pathlib import Path
import re
import unicodedata

ROOT = Path(__file__).resolve().parents[1]
SUFFIXES = ('лар', 'ләр', 'тар', 'тәр', 'дар', 'дәр', 'ҙар', 'ҙәр')
GRAMMAR_URL = 'https://basic.bashlang.ru/grammar/paragraph/7/'
RAN_URL = 'https://minlang.iling-ran.ru/grammar/bashkirskiy-grammaticheskiy-ocherk'
# Explicit editorial interpretations are separate from automatic suffix matching.
# Confidence describes morphology, not suitability for deletion.
REVIEWED = {
    'АҠТАР': ('аҡ', 'substantivized_plural', 'high', 'Субстантивированное «белые»; название группы людей, не обычный предмет.'),
    'БАҪМАҠТАР': ('баҫмаҡ', 'lexicalized_plural', 'high', 'Форма с -тар от баҫмаҡ «клавиша», но словарное значение целого слова — «клавиатура». Автоматическая замена на основу теряет значение.'),
    'БОРОНҒОЛАР': ('боронғо', 'substantivized_plural', 'high', '«Древние люди»: субстантивированная форма от боронғо, окончание -лар.'),
    'ИМЕҘЕҮСЕЛӘР': ('имеҙеүсе', 'taxonomic_plural', 'high', 'Название класса животных «млекопитающие», форма на -ләр. Таксон требует отдельного правила игры.'),
    'ЙӘШЕЛДӘР': ('йәшел', 'substantivized_plural', 'high', 'Субстантивированное «зелёные», окончание -дәр.'),
    'ЙӘШТӘР': ('йәш', 'lexicalized_pluralia_tantum', 'high', 'Словарное pluralia tantum от прилагательного йәш, собирательное «молодёжь». Это не просто множественное от существительных йәш «возраст/слеза».'),
    'КАНИКУЛДАР': ('каникул', 'borrowed_pluralia_tantum', 'high', 'Явный башкирский аффикс -дар; русский эквивалент «каникулы» — pluralia tantum. Основа КАНИКУЛ была удалена ранее по отдельному запросу.'),
    'ҠАЛҠАНЛЫЛАР': ('ҡалҡанлы', 'taxonomic_plural', 'high', '«Броненосцы», название группы животных с -лар; окончательное решение зависит от допустимости названий таксонов.'),
    'ТАҘҘАР': ('таҙ', 'clan_name_plural', 'medium', 'В базе название башкирского рода тазлар. Формально -ҙар; название собственное/родовое, ранее оставлено по правилу о башкирских родах.'),
    'УМЫРТҠАЛЫЛАР': ('умыртҡалы', 'taxonomic_plural', 'high', '«Позвоночные», название группы животных с -лар; это не русская ошибка перевода.'),
    'ҺӨТИМӘРҘӘР': ('һөтимәр', 'taxonomic_plural', 'high', 'Форма на -ҙәр; в базе есть ҺӨТИМӘР «млекопитающее». Название класса животных.'),
    'ҺӘНӘКСЕЛӘР': ('һәнәксе', 'historical_group_plural', 'high', 'Участники исторического мятежа: название группы с -ләр.'),
    'ЯҢЫЛАР': ('яңы', 'clan_name_plural', 'medium', 'В базе названия подразделений башкир. Формально -лар; как название рода не является обычной свободной формой множественного числа.'),
}
SPECIAL_EVIDENCE = {
    'ЙӘШТӘР': ['https://en.wiktionary.org/wiki/йәштәр'],
    'БАҪМАҠТАР': ['https://ru.wiktionary.org/wiki/баҫмаҡтар'],
    'ҺӨТИМӘРҘӘР': ['https://ru.wiktionary.org/wiki/һөтимәрҙәр', 'https://ru.wiktionary.org/wiki/һөтимәр'],
    'ИМЕҘЕҮСЕЛӘР': ['https://kitaptar.bashkort.org/storage/books/UCqvVMwACVBQ8GnyTEu8yznXz5Fc3TL70HTBeYNr.pdf'],
    'ҠАЛҠАНЛЫЛАР': ['https://kitaptar.bashkort.org/files/Башҡортса%20-%20русса%20һүҙлек.pdf'],
}
COINCIDENCES = {
    'АЙҘАР': 'Самостоятельное «хохол, чуб»; наличие АЙ «луна» не доказывает множественное.',
    'АЛДАР': '«Обманщик»; совпадение с АЛ + дар не означает множественное.',
    'АҪТАР': '«Подкладка»; совпадение с АҪ + тар не означает множественное.',
    'ДАРДАР': '«Ясень»; совпадение с ДАР + дар не означает множественное.',
    'ДӘФТӘР': '«Тетрадь»: начальная форма приведена и в учебной грамматике, окончание — часть основы.',
    'ҠАНТАР': '«Короткие поводья»; регулярное множественное от ҡан было бы ҡандар, не ҡантар.',
    'САПТАР': 'Название масти лошади; совпадение с САП + тар не означает множественное.',
    'ТИПТӘР': 'Словарное «тептяр»; совпадение с ТИП + тәр не означает множественное.',
    'ХӨКӨМДАР': '«Правитель»; совпадение с ХӨКӨМ + дар не означает множественное.',
    'ТАРАФДАР': '«Единомышленник, приверженец»; не множественное от тараф.',
}


def normalize(text):
    return unicodedata.normalize('NFC', text).casefold().replace('\u0301', '')


def expected_suffix(stem):
    """Conservative spelling check, not a morphological analyser of Bashkir."""
    stem = normalize(stem)
    if not stem:
        return None
    consonant = next((c for letters, c in [('аәыеоөяэ', 'л'), ('лмнңзж', 'д'),
        ('бвгдкҡпсҫтфхһцчшщ', 'т'), ('ҙйруүию', 'ҙ')] if stem[-1] in letters), None)
    vowel = next((v for v in reversed(stem) if v in 'аоуыәөүеиэяюё'), None)
    if not consonant or not vowel:
        return None
    return consonant + ('әр' if vowel in 'әөүеиэ' else 'ар')


def suffix_analysis(word, words):
    lower = normalize(word)
    suffix = next((s for s in SUFFIXES if lower.endswith(s)), None)
    if suffix is None:
        return None
    stem = lower[:-len(suffix)]
    expected = expected_suffix(stem)
    return {'suffix': suffix, 'possibleStem': stem, 'stemInDataset': stem.upper() in words,
            'expectedRegularSuffix': expected, 'spellingCompatible': expected == suffix}


def glosses(entry, language='ru'):
    return [s['text'] for s in entry.get('definitions', {}).get(language, []) if s.get('text')]


def sources(entry):
    # Keep local provenance too: some source URLs have since moved.
    return entry.get('sources', [])


def russian_segments(text):
    text = normalize(text)
    text = re.sub(r'\([^()]*\)', '', text)
    for clause in re.split(r'[;/]|\s+или\s+', text):
        clause = re.sub(r'^(?:(?:[а-яё]+\.)\s*)+', '', clause.strip())
        fragments = [s.strip(' .:«»"') for s in clause.split(',')]
        # An entire comma list must look like short equivalents. Otherwise a
        # trailing word in a long description ("... украшенный подвесками")
        # could become a misleading standalone gloss.
        if any(not re.fullmatch(r'[а-яё]+(?:[ -][а-яё]+){0,3}', s) for s in fragments):
            continue
        if any(re.search(r'\b(?:в|на|с|со|из|для|к|по|при|от|до|о|об|его|её|их|или|и|который|это)\b', s) for s in fragments):
            continue
        yield from fragments


class RussianNumberAnalyser:
    def __init__(self):
        import pymorphy3
        self.morph = pymorphy3.MorphAnalyzer()

    @lru_cache(maxsize=None)
    def word(self, word):
        # Dictionary-backed analyses only; guesses create false positives for
        # Bashkir headwords, misspellings, names and long explanatory prose.
        return tuple(p for p in self.morph.parse(word) if p.is_known and
                     p.tag.POS in {'NOUN', 'ADJF', 'PRTF'} and not
                     {'Name', 'Surn', 'Patr', 'Geox', 'Abbr'}.intersection(p.tag.grammemes))

    def analyse(self, segment):
        tokens = segment.split()
        parsed = [self.word(token) for token in tokens]
        if not parsed or any(not ps for ps in parsed):
            return None
        # Head-final nominal phrases only, with inflecting modifiers before it.
        # Reject genitive complements/descriptions instead of tagging an
        # incidental word in "часть тела животных" as the translation's number.
        head = [p for p in parsed[-1] if p.tag.POS == 'NOUN']
        if not head:
            # Substantivized scientific/group labels such as "позвоночные".
            head = [p for p in parsed[-1] if p.tag.POS in {'ADJF', 'PRTF'}] if len(tokens) == 1 else []
        if not head:
            return None
        modifiers = [[p for p in ps if p.tag.POS in {'ADJF', 'PRTF'}] for ps in parsed[:-1]]
        if any(not ps for ps in modifiers):
            return None
        plural = [p for p in head if p.tag.number == 'plur']
        # Headword audit: require a nominative-plural reading. A bare genitive
        # plural coincidence ("белок" = squirrels' vs protein) is not useful.
        if not any(p.tag.case == 'nomn' for p in plural) or any(not any(p.tag.number == 'plur' and p.tag.case == 'nomn' for p in ps) for ps in modifiers):
            return None
        singular = [p for p in head if p.tag.number == 'sing']
        singular_possible = bool(singular) and all(any(p.tag.number == 'sing' for p in ps) for ps in modifiers)
        if singular_possible:
            category = 'ambiguous_number'
        elif all('Pltm' in p.tag for p in plural):
            category = 'russian_pluralia_tantum'
        else:
            category = 'russian_plural'
        return {'text': segment, 'category': category,
                'confidence': 'low' if singular_possible else 'medium',
                'singularAlsoPossible': singular_possible,
                'pluraliaTantumAlsoPossible': any('Pltm' in p.tag for p in plural),
                'analyses': sorted({(p.normal_form, str(p.tag)) for p in head})}


def audit(full, analyser):
    entries = full['entries']
    words = {e['display'] for e in entries}
    bashkir, coincidences, other_forms, collectives, russian = [], [], [], [], []
    segments_seen = 0
    for entry in entries:
        word = entry['display']
        record = {'word': word, 'russian': glosses(entry), 'sourceRecords': sources(entry)}
        suffix = suffix_analysis(word, words)
        if word in REVIEWED:
            stem, kind, confidence, note = REVIEWED[word]
            bashkir.append({**record, **suffix, 'reviewedStem': stem, 'category': kind,
                'confidence': confidence, 'note': note,
                'evidenceUrls': [GRAMMAR_URL] + SPECIAL_EVIDENCE.get(word, [])})
        elif suffix:
            coincidences.append({**record, **suffix,
                'category': 'lexical_coincidence' if word in COINCIDENCES else 'unconfirmed_suffix_match',
                'confidence': 'not_established',
                'note': COINCIDENCES.get(word, 'Совпадение букв не подтверждает морфему. Требуется отдельная словарная проверка; не включено в подтверждённые формы.')})
        explicit_forms = [s for s in glosses(entry, 'en') if re.search(r'possessed nominative|plural of|singular of', s, re.I)]
        if explicit_forms:
            other_forms.append({**record, 'englishEvidence': explicit_forms,
                'category': 'possessed_singular' if any('possessed nominative' in s for s in explicit_forms) else 'semantic_plural',
                'note': 'Словарная помета формы/употребления; не автоматическое основание для удаления.'})
        collective = [s for lang in ('ru', 'en') for s in glosses(entry, lang)
                      if re.search(r'собир\.|\bcollective\b', s, re.I)]
        if collective:
            collectives.append({**record, 'explicitEvidence': collective,
                'note': 'Собирательное значение не равно морфологическому множественному числу.'})
        found = {}
        for text in glosses(entry):
            for segment in russian_segments(text):
                segments_seen += 1
                analysed = analyser.analyse(segment)
                if analysed:
                    found[segment] = analysed
        if found:
            russian.append({**record, 'matches': sorted(found.values(), key=lambda x: x['text']),
                'bashkirPluralIndependentlyReviewed': word in REVIEWED,
                'note': 'Число русского эквивалента не определяет число башкирской леммы.'})
    return {'datasetRevision': full['datasetRevision'], 'entryCount': len(entries),
            'russianSegmentsAnalysed': segments_seen,
            'bashkirPluralCandidates': bashkir, 'suffixCoincidencesAndUnresolved': coincidences,
            'otherExplicitForms': other_forms, 'explicitCollectives': collectives,
            'russianTranslationCandidates': russian,
            'controls': {text: analyser.analyse(text) for text in ['дочери', 'ножницы', 'дочь', 'короткие поводья']}}


def cell(value):
    if isinstance(value, list):
        value = '; '.join(map(str, value))
    return str(value).replace('|', '\\|').replace('\n', ' ')


def table(headers, rows):
    return '\n'.join(['| ' + ' | '.join(headers) + ' |',
                      '| ' + ' | '.join(['---'] * len(headers)) + ' |'] +
                     ['| ' + ' | '.join(cell(v) for v in row) + ' |' for row in rows])


def markdown(data):
    ba = data['bashkirPluralCandidates']
    ru = data['russianTranslationCandidates']
    out = [f"# Проверка числа и форм слов — {data['date']}",
        f"Проверено слов: **{data['entryCount']}**. Ревизия: `{data['datasetRevision']}`. Скрипт не удаляет слова.",
        'По прямому запросу удалено только **ҠЫҘЫ**. Это притяжательная форма единственного числа от ҠЫҘ («его/её дочь»), также компонент женского отчества. Русское «дочери» само по себе неоднозначно: родительный/дательный/предложный единственного либо именительный множественного. [Статья ҠЫҘЫ](https://en.wiktionary.org/w/index.php?title=ҡыҙы&oldid=81594358), [склонение «дочь»](https://ru.wiktionary.org/wiki/дочь).',
        '## Башкирские формы для решения пользователем',
        f'Кандидатов: **{len(ba)}**. Уверенность ниже относится к морфологическому разбору, а не к необходимости удаления. Здесь есть названия групп, таксонов, родов и лексикализованные слова; все сохранены.',
        table(['Слово', 'Перевод из базы', 'Разбор', 'Уверенность', 'Основание / оговорка'],
              [(r['word'], r['russian'], r['reviewedStem']+' + '+r['suffix'], 'высокая' if r['confidence']=='high' else 'средняя', r['note']) for r in ba]),
        f'[Учебная грамматика]({GRAMMAR_URL}) описывает восемь вариантов аффикса и зависимость от основы. [Очерк Института языкознания РАН]({RAN_URL}) различает число, обладание и падеж. Индивидуальные источники и provenance каждой записи находятся в JSON.',
        '[ЙӘШТӘР](https://en.wiktionary.org/wiki/йәштәр) отдельно описано как субстантивированное pluralia tantum с собирательным значением; [БАҪМАҠТАР](https://ru.wiktionary.org/wiki/баҫмаҡтар) имеет самостоятельное значение «клавиатура».',
        'Таксономические значения дополнительно сверены по [башкирско-русскому словарю: һөтимәрҙәр / имеҙеүселәр](https://kitaptar.bashkort.org/storage/books/UCqvVMwACVBQ8GnyTEu8yznXz5Fc3TL70HTBeYNr.pdf) и [словарной статье ҡалҡанлылар](https://kitaptar.bashkort.org/files/Башҡортса%20-%20русса%20һүҙлек.pdf).',
        '## Другие формы с прямой пометой в источнике',
        table(['Слово', 'Перевод', 'Помета источника'], [(r['word'],r['russian'],r['englishEvidence']) for r in data['otherExplicitForms']]),
        'УЛЫ — оставленная притяжательная форма единственного числа («его/её сын»), аналогичная удалённому ҠЫҘЫ. ХАЛЫҠ описывает людей/народ; семантическое употребление как множественного от «человек» не превращает его в регулярную форму с суффиксом.',
        '## Явные собирательные значения',
        table(['Слово', 'Перевод', 'Помета'], [(r['word'],r['russian'],r['explicitEvidence']) for r in data['explicitCollectives']]),
        '## Множественное число только в русском переводе',
        'Это список для проверки переводов. Он **не** доказывает множественное число башкирского слова. ҠАЙСЫ «ножницы» сохранено: русский эквивалент — pluralia tantum, а башкирская запись не содержит аффикса множественного числа. Русский анализатор не применяется к башкирским заголовкам: например, БОЛОТ «облако» нельзя считать русским родительным множественного от «болото».']
    labels = [('russian_plural', 'Русский разбор допускает только множественное число'),
              ('russian_pluralia_tantum', 'Русские pluralia tantum'),
              ('ambiguous_number', 'Неоднозначность: возможны единственное и множественное')]
    for category, title in labels:
        rows = [(r['word'], [m['text'] for m in r['matches'] if m['category']==category]) for r in ru]
        rows = [r for r in rows if r[1]]
        out += ['### '+title, f'Слов: {len(rows)}. '+('Уверенность низкая; это не подтверждённые plurals.' if category=='ambiguous_number' else 'Уверенность средняя: автоматический словарный разбор, без полного контекстного анализа.'), table(['Башкирская запись', 'Русские формы'], rows)]
    out += ['## Совпадения с окончаниями: не подтверждённое множественное число',
        f"Совпадений: {len(data['suffixCoincidencesAndUnresolved'])}. Ни совпадение последних трёх букв, ни существование похожей основы в базе недостаточны для удаления.",
        table(['Слово', 'Перевод', 'Предполагаемая основа', 'Проверка'],
              [(r['word'],r['russian'] or 'нет русского перевода',r['possibleStem'] or 'пустая',r['note']) for r in data['suffixCoincidencesAndUnresolved']]),
        '## Воспроизведение и ограничения',
        '```sh\npython -m pip install -r scripts/requirements-dictionary-audit.txt\npython scripts/audit-plurals.py\npython -m unittest discover -s tests -p "dictionary*_test.py"\n```',
        f"Русских коротких фрагментов обработано: {data['russianSegmentsAnalysed']}. Версии анализатора и словаря зафиксированы в requirements и JSON. [pymorphy3](https://pypi.org/project/pymorphy3/) использует морфологический словарь; учитываются только известные словарю разборы.",
        'Проверка башкирских окончаний — орфографическая эвристика и отдельный проверенный список. Она не является полным морфологическим анализатором: формы с дополнительными притяжательными/падежными аффиксами, нерегулярные формы и ошибки импорта могут быть пропущены. Обнаруженные совпадения перечислены целиком. Отдельное совпадение с основой не устанавливает семантическую связь.',
        'Русский анализ выполняется для отдельных слов и коротких именных сочетаний до четырёх слов. Требуется хотя бы один разбор в именительном множественного, поскольку проверяются словарные формы; только косвенные множественные (например, «дочерям») в этот список не входят. Длинные описания, неизвестные формы, собственные имена и конструкции с падежными дополнениями пропускаются. Скобки и известные сокращённые пометы удаляются; перечень через запятую берётся только если все части похожи на короткие эквиваленты. Вне контекста возможны омонимия, субстантивация прилагательных и неверные разборы. «Только множественное» означает только рассмотренные словарные разборы фрагмента, а не лингвистическое доказательство или требование удаления.',
        'Собирательное значение, таксономическое название и pluralia tantum рассматриваются отдельно. Сохранённые записи не исправлялись. В JSON доступны полные исходные переводы, источники, грамматические теги и основания.']
    return '\n\n'.join(out)+'\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, default=ROOT/'public/data/nouns.full.json')
    parser.add_argument('--output', type=Path, default=ROOT/'reports/plural-audit-2026-11-13')
    parser.add_argument('--date', default='2026-11-13')
    args = parser.parse_args()
    try:
        analyser = RussianNumberAnalyser()
    except ImportError as exc:
        raise SystemExit('Install scripts/requirements-dictionary-audit.txt before running this audit.') from exc
    full = json.loads(args.input.read_text('utf-8'))
    data = audit(full, analyser)
    data.update(date=args.date, auditVersion=1, removedAutomatically=False,
                dependencies={name: importlib.metadata.version(name) for name in
                              ['pymorphy3', 'pymorphy3-dicts-ru', 'dawg2-python']},
                grammarSources=[GRAMMAR_URL,RAN_URL])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.with_suffix('.json').write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n','utf-8')
    args.output.with_suffix('.md').write_text(markdown(data),'utf-8')
    counts = Counter(m['category'] for r in data['russianTranslationCandidates'] for m in r['matches'])
    print(json.dumps({'bashkirCandidates':len(data['bashkirPluralCandidates']),
        'suffixCoincidences':len(data['suffixCoincidencesAndUnresolved']),
        'russianCandidateWords':len(data['russianTranslationCandidates']),
        'russianFormsByCategory':dict(counts)},ensure_ascii=False))


if __name__ == '__main__':
    main()
