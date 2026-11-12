"""Reproduce the 2026-11-12 editorial audit without changing game data.

python scripts/audit-dictionary.py --baseline-ref v2.9.5
Only insertions/deletions count as near translations; substitutions never do.
"""
import argparse
import collections
import json
import pathlib
import re
import subprocess
import unicodedata

ROOT = pathlib.Path(__file__).resolve().parents[1]
POLICY = json.loads((ROOT / 'scripts/dictionary-policy.json').read_text('utf-8'))
LABEL = re.compile(r'^(?:(?:[12]\.?\s+)|(?:(?:зоол|ботан|бот|ихтиол|орнитол|энтомол|анат|астрон|астр|геогр|геол|матем|мат|лингв|грам|религ|рел|мифол|этногр|кулин|спорт|театр|архит|истор|ист|устар|уст|перен|разг|диал|книжн|прост|бран|шутл|поэт|тех|мед|вет|физ|хим|муз|советск|мн|ч)\.\s*))', re.I)
WORD = re.compile(r'^[а-яёғҙҡңөҫүһә]+$')


def normalize(value):
    return unicodedata.normalize('NFC', value).casefold().replace('\u0301', '')


def alternatives(text):
    """Whole single-word gloss alternatives, never arbitrary tokens in a sentence.

    Balanced parenthetical explanations and known leading dictionary labels are
    removed. е/ё, Bashkir letters, and Latin lookalikes remain distinct.
    """
    text = normalize(text)
    while re.search(r'\([^()]*\)', text):
        text = re.sub(r'\([^()]*\)', '', text)
    for part in re.split(r'[,;/]|\s+или\s+', text):
        part = part.strip()
        while LABEL.match(part):
            part = LABEL.sub('', part, count=1).strip()
        part = part.strip(' .:«»“”\"\'')
        if WORD.fullmatch(part):
            yield part


def insertion_difference(first, second):
    """Return inserted letters/positions if shorter is a subsequence of longer."""
    first, second = normalize(first), normalize(second)
    if len(first) == len(second):
        return None
    short, long = sorted((first, second), key=len)
    i, insertions = 0, []
    for pos, letter in enumerate(long):
        if i < len(short) and short[i] == letter:
            i += 1
        else:
            insertions.append({'position': pos + 1, 'letter': letter})
    if i != len(short):
        return None
    return {'difference': len(long) - len(short), 'longer': long,
            'extraLetters': ''.join(item['letter'] for item in insertions),
            'insertions': insertions}


def pairs(entries):
    result, seen = [], set()
    for entry in entries:
        word = normalize(entry['word'])
        for sense in entry.get('definitions', {}).get('ru', []):
            for translation in alternatives(sense['text']):
                match = insertion_difference(word, translation)
                if not match or match['difference'] not in (1, 2, 3):
                    continue
                if match['difference'] > 1 and len(word) < 6:
                    continue
                key = (word, translation)
                if key in seen:
                    continue
                seen.add(key)
                result.append({'word': entry['display'], 'translation': translation,
                               'wordLength': len(word), 'translationLength': len(translation),
                               'bothAtLeastSix': min(len(word), len(translation)) >= 6,
                               'sourceText': sense['text'], 'sourceId': sense.get('sourceId'),
                               'sourceUrl': sense.get('url'), 'pdfPage': sense.get('pdfPage'), **match})
    return sorted(result, key=lambda p: (p['difference'], p['word'], p['translation']))


CURATED = {
    'ҺЫҢАР': ('meaning_part_of_speech', '«Один» при noun; проверить именное значение (половина пары и т. п.), перевод не раскрывает его.'),
    'ХӨСӘЙЕН': ('proper_name', 'Единственный перевод «хусаин»; вероятно личное имя с потерянным регистром, требуется сверка.'),
    'ҠЫҘЫ': ('inflected_headword', 'Притяжательная форма от ҡыҙ («его/её дочь»), также компонент отчества; проверить допустимость формы вместо начальной. Источник: https://en.wiktionary.org/wiki/ҡыҙы'),
    'УЛЫ': ('inflected_headword', 'Форма от ул («его/её сын»), также компонент отчества; проверить допустимость формы вместо начальной.'),
    'ИРТӘН': ('meaning_part_of_speech', 'Единственный перевод «Завтра»; проверить время суток/наречное значение и корректность русского перевода.'),
    'ЙОМРО': ('meaning_part_of_speech', 'Перевод начинается с «большой, круглый по количеству ...»; именное значение не раскрыто.'),
    'ТӨКЛӘҪ': ('proper_name', '«Святой Баба Туклес» — собственное имя религиозного/фольклорного персонажа.'),
    'ҺОМАЙ': ('proper_name', '«Хумай» — имя мифологического персонажа; требуется решение о допустимости.'),
    'БЕЛЕШ': ('ethnonym_boundary', '«Знакомый, знакомец русский по отношению к башкирам»: есть обычное значение и историческое обозначение русских; значение нельзя надежно разделить автоматически.'),
    'ТИПТӘР': ('ethnonym_boundary', '«Тептяр» — историческая этносословная группа региона; не приравнивается автоматически к названию другого народа.'),
    'ҠЫПСАҠ': ('ethnonym_boundary', '«Кыпчак» — также название башкирского племени; сохранено в рамках исключения для башкирских родов, требуется отдельное решение об историческом этнониме.'),
    'БУРАЛАНЫУ': ('meaning_part_of_speech', 'Перевод «горячиться» — глагол; проверить отглагольное существительное и перевод, а не удалять по одному суффиксу.'),
    'ҠАПРАЙТЫУ': ('meaning_part_of_speech', 'Перевод «вздувать» — глагол; проверить наличие именного значения.'),
    'ФИРҮЗӘ': ('meaning_part_of_speech', 'Перевод «бирюзoвый» содержит латинскую o и прилагательное; проверить значение «бирюза».'),
    'ХОЗУР': ('meaning_part_of_speech', 'Перевод «спокoйный» содержит латинскую o и прилагательное.'),
    'ФӘЛӘН': ('meaning_part_of_speech', '«И так далее, и тому подобное» — не предметный перевод; проверить часть речи.'),
    'ИЛЕ': ('meaning_part_of_speech', '«Толщиной, шириной в [один] палец» — измерительный оборот; нужна проверка именного употребления.'),
    'БУЛҒАН': ('meaning_part_of_speech', '«Проворный» при метке noun: проверить именное значение/перенос части речи.'),
    'ЙОТЛОҠ': ('meaning_part_of_speech', '«Голодный» при метке noun: проверить перевод.'),
    'СОНОР': ('meaning_part_of_speech', '«Сонорный» при метке noun: возможно термин «сонорный согласный», нужен полный перевод.'),
    'ФРИКАТИВ': ('meaning_part_of_speech', '«Фрикативный» при метке noun: возможно название типа согласного, нужен полный перевод.'),
    'ҺӨТСӨЛӨК': ('meaning_part_of_speech', '«Молочный» при метке noun: проверить пропущенное существительное.'),
    'ЙӘМӘҒӘТСЕЛЕК': ('meaning_form', 'Перевод «общественности» дан в косвенном падеже вместо словарной формы.'),
    'ҠЫРҠҠЫ': ('meaning_format', 'Перевод «НОЖНИЦЫ ДЛЯ стрижки овец» содержит случайный верхний регистр.'),
    'КЕЛӘ': ('ocr', '«Гцеколда; JJj latch; catch» — явные следы OCR/английской колонки.'),
    'БЙЗНЕС': ('ocr', 'Подозрительное Й вместо И; нет перевода. Сверить написание с источником.'),
    'МАГЙСТР': ('ocr', 'Подозрительное Й вместо И; нет перевода. Сверить написание с источником.'),
    'ОТЛЙЧНИЦА': ('ocr', 'Подозрительное Й вместо И; нет перевода. Сверить написание с источником.'),
    'МЕНЫНЕВИЗМ': ('ocr', 'Подозрительное распознавание Ш как ЫН; нет перевода.'),
    'МЕНЫПЕВИК': ('ocr', 'Подозрительное распознавание Ш как ЫП; нет перевода.'),
    'ДЖЕНТЕЛЬМЁН': ('ocr', 'Вероятное повреждение заимствованного заголовка; нет перевода.'),
    'КУРУРУЗ': ('ocr', 'Подозрительная последовательность букв в заголовке; нет перевода.'),
    'СИМТОМАТИКА': ('ocr', 'Возможен пропуск буквы в заимствовании; нет перевода.'),
    'ФЛЮРОГРАФИЯ': ('ocr', 'Возможен пропуск буквы в заимствовании; нет перевода.'),
    'ПОЛИОМЕЛИТ': ('ocr', 'Возможен пропуск буквы в заимствовании; нет перевода.'),
    'ПОЧТАМПТ': ('ocr', 'Возможна лишняя буква в заимствовании; нет перевода.'),
    'АБСТРАКТ': ('unconfirmed_noun', 'Нет перевода; форма может быть прилагательным/наречием, noun не подтвержден толкованием.'),
    'ҮҘЛЕГЕНӘН': ('unconfirmed_noun', 'Нет перевода; форма выглядит как наречное выражение, требуется сверка.'),
    'БЕРӘЙҺЕ': ('unconfirmed_noun', 'Нет перевода; вероятно местоименная форма, требуется сверка.'),
    'ҺУҢҒЫ': ('unconfirmed_noun', 'Нет перевода; вероятно прилагательное, требуется сверка именного употребления.'),
    'ОТЛИЧНО': ('unconfirmed_noun', 'Нет перевода; может быть наречием либо субстантивированной школьной отметкой.'),
    'АЭРОФЛОТ': ('name_or_abbreviation', 'Нет перевода; название организации/сокращенное наименование.'),
    'СӨЛӘЙМӘН': ('proper_name', 'Нет перевода; вероятно личное имя. Проверить возможный нарицательный омоним.'),
    'ХӘЙЕРНИСА': ('proper_name', 'Нет перевода; вероятно личное имя. Проверить возможный нарицательный омоним.'),
    'ЯБРАИЛ': ('proper_name', 'Нет перевода; вероятно имя. Проверить исходную статью.'),
    'ИНД': ('proper_name', 'Нет перевода. В словаре 1958 есть Инд как река; запись из орфографического источника не уточняет значение.'),
    'КАМПУЧИ': ('possible_ethnonym', 'Нет перевода; вероятное историческое наименование кампучийца, но значение этой записи пока не подтверждено. Не удалено предположительно.'),
    'ТӨРКИ': ('ethnonym_boundary', '«Тюрки; тюрки (письменность)»: общая группа включает башкир, есть значение письменности. Не отнесено автоматически к другим народам.'),
}


def review(entries):
    flags = []
    for entry in entries:
        senses = entry.get('definitions', {}).get('ru', [])
        notes = []
        if entry['display'] in CURATED:
            notes.append(CURATED[entry['display']])
        if not any(s.get('text', '').strip() for s in senses):
            notes.append(('missing_russian', 'Нет русского перевода: слово принимается бонусом, но не может стать целевым словом поля.'))
        texts = [s['text'] for s in senses]
        if any(re.search(r'(?i)(?<=[а-яё])[a-z]|[a-z](?=[а-яё])', t) for t in texts):
            notes.append(('mixed_alphabet', 'Внутри русского слова встречается латинская буква; проверить OCR.'))
        if any(re.search(r'(?:\b[ПИЙНР]\s*[.,;]\s*[:;]|\b(?:EL|PL|JJj)|\b14\.:|E l)', t) for t in texts):
            notes.append(('foreign_column', 'В русский перевод попала разметка/текст английской колонки; сверить и очистить.'))
        if any('созвездие' in t and ('прописной' in t or 'Тельца' in t) for t in texts):
            notes.append(('proper_name_sense', 'Есть значение собственного имени созвездия. Другие нарицательные значения могут быть корректны.'))
        elif any(re.fullmatch(r'[А-ЯЁ][а-яё]+', t) for t in texts) and entry['display'] not in ('ИРТӘН',):
            notes.append(('proper_name_sense', 'Русский однословный перевод дан с прописной буквы: возможно личное имя, мифологическое имя или астроним. Другие значения могут быть нарицательными.'))
        if notes:
            flags.append({'word': entry['display'], 'id': entry['id'], 'meanings': texts,
                          'isTarget': bool(texts), 'issues': [{'category': k, 'reason': v} for k, v in notes],
                          'sources': entry.get('sources', [])})
    return flags


def table(headers, rows):
    esc = lambda v: str(v).replace('|', '\\|').replace('\n', ' ')
    return '\n'.join(['| ' + ' | '.join(headers) + ' |', '| ' + ' | '.join('---' for _ in headers) + ' |'] +
                     ['| ' + ' | '.join(esc(v) for v in row) + ' |' for row in rows])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline-ref', default='v2.9.5')
    args = parser.parse_args()
    full = json.loads((ROOT / 'public/data/nouns.full.json').read_text('utf-8'))
    raw = json.loads((ROOT / 'public/data/game-lexicon.nouns.json').read_text('utf-8'))
    baseline = json.loads(subprocess.check_output(['git', 'show', f'{args.baseline_ref}:public/data/nouns.full.json'], cwd=ROOT))
    current_words = {e['display'] for e in full['entries']}
    removed = [e for e in baseline['entries'] if e['display'] not in current_words]
    remaining_pairs = pairs(full['entries'])
    removed_pairs = [dict(p, status='removed') for p in pairs(removed)]
    flags = review(full['entries'])
    counts = collections.Counter(issue['category'] for item in flags for issue in item['issues'])
    report_dir = ROOT / 'reports'
    report_dir.mkdir(exist_ok=True)
    def save(name, data):
        (report_dir / name).write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n', 'utf-8')
    save('dictionary-removed-2026-11-12.json', {'baseline': args.baseline_ref, 'datasetRevision': full['datasetRevision'], 'removedEntries': removed})
    save('near-translations-2026-11-12.json', {'datasetRevision': full['datasetRevision'], 'rule': 'NFC, casefold, accent marks ignored; one entire single-word Russian alternative; shorter must be a subsequence of longer; difference 1, or 2/3 with Bashkir word length >=6. е/ё and Bashkir letters stay distinct.', 'pairs': remaining_pairs, 'removedPairs': removed_pairs})
    save('dictionary-review-2026-11-12.json', {'datasetRevision': full['datasetRevision'], 'wordCount': raw['wordCount'], 'targetCount': raw['targetWordCount'], 'flaggedWords': len(flags), 'categoryCounts': counts, 'reviewIsNotProofOfInvalidity': True, 'candidates': flags})
    md = ['# Слово и перевод: только добавление/удаление букв', '',
          f'База после очистки 12 ноября 2026: {raw["wordCount"]} слов, ревизия `{full["datasetRevision"]}`.', '',
          'Проверены все непустые русские значения. Сравниваются целые однословные варианты, разделённые запятыми, точкой с запятой, косой чертой или «или». Словарные пометы и пояснения в скобках не участвуют. Регистр и знак ударения не учитываются; е/ё, ҡ/к, һ/с и другие разные буквы не объединяются. Слова не транслитерируются. Замены и перестановки букв не допускаются: короткая строка должна полностью читаться в длинной в исходном порядке.', '',
          'Для различия в 2 и 3 буквы ограничение «от 6 букв» применено к башкирскому слову. Звёздочкой отмечен перевод короче 6 букв; без этих строк получаются списки, где обе стороны не короче 6. Подобие написания само по себе не доказывает ошибку или заимствование; эти слова не удалены.', '']
    for delta in (1, 2, 3):
        group = [p for p in remaining_pairs if p['difference'] == delta]
        strict = sum(p['bothAtLeastSix'] for p in group)
        md += [f'## Разница {delta}. Пар: {len(group)}; слов: {len(set(p["word"] for p in group))}', '',
               (f'Пар с обеими сторонами от 6 букв: {strict}.' if delta > 1 else 'Длина слова не ограничена.'), '',
               table(['Башкирское слово', 'Русский вариант', 'Длины', 'Лишние буквы в длинном слове'],
                     [(p['word'], p['translation'] + (' *' if delta > 1 and not p['bothAtLeastSix'] else ''), f'{p["wordLength"]}/{p["translationLength"]}', p['extraLetters']) for p in group]), '']
    md += ['## Совпадения в удалённых записях', '', 'Не входят в списки действующей базы выше; приведены для проверки исходной версии 2.9.5.', '',
           table(['Слово', 'Перевод', 'Разница'], [(p['word'], p['translation'], p['difference']) for p in removed_pairs]), '',
           'В JSON сохранены исходный текст значения, источник, длины и позиции лишних букв. Воспроизведение: `python scripts/audit-dictionary.py --baseline-ref v2.9.5`.', '']
    (report_dir / 'near-translations-2026-11-12.md').write_text('\n'.join(md), 'utf-8')
    md = ['# Проверка словаря — 12 ноября 2026', '',
          f'До очистки: {len(baseline["entries"])} слов. Удалено: {len(removed)}. Осталось: {raw["wordCount"]}, из них {raw["targetWordCount"]} с русскими значениями. Для проверки отмечено {len(flags)} уникальных слов; категории могут пересекаться.', '',
          'Это проверка структуры, всех сохранённых значений и подозрительных записей, а не полная ручная лингвистическая сертификация. Во всех записях уже стоит метка noun; она не гарантирует правильность извлечённого значения. Остальные кандидаты сохранены до редакторского решения.', '',
          '## Удалённые слова из запроса', '',
          table(['В запросе', 'Игровое написание', 'Значения до удаления'], [(label, word, '; '.join(s['text'] for e in removed if e['display'] == word for s in e['definitions']['ru'])) for label, word in POLICY['requestedRemovalAliases'].items()]), '',
          '«Калын» сопоставлен с ҠАЛЫН; отдельное ҠАЛЫМ (вено, калым, выкуп) сохранено. «Сарык» сопоставлен с САРЫҠ (чарык), а ҺАРЫҠ (овца) сохранено. УЛАҠ и ӘРЙӘ, у которых «ларь» лишь одно из значений, сохранены. Точные русские написания запроса и башкирские соответствия внесены в запрет повторного импорта.', '',
          '## Названия других народов и жителей', '',
          f'Исключено {sum(e["display"] in POLICY["excludedEthnonyms"] for e in removed)} слов с этническими/национальными/географическими названиями людей; БОЛҒАР, ТАТАР и БАЛҠАР одновременно входят в список выше. Названия народов грамматически являются существительными; здесь действует отдельное правило игры. Башкирские самоназвания и роды сохранены.', '',
          table(['Удалённое слово', 'Основание'], [(e['display'], POLICY['excludedEthnonyms'][e['display']]) for e in removed if e['display'] in POLICY['excludedEthnonyms']]), '',
          'Удаление выполнено по целому игровому слову: вместе с ДАТ исключены значения «верность, месть, горечь, боль»; с ҠАҘАҠ — «гвоздь», с ҠЫТАЙ — «бусы», с ЯҠУТ — «яхонт», с УҒЫҘ — «молозиво». Это омонимы названий других народов.', '',
          f'Записи без русских значений, сверенные отдельно: {len(POLICY["ethnonymEvidence"])}.', '']
    md += [f'- {word}: {e["meaning"]}; [{e["locator"]}]({e["url"]}).' for word, e in POLICY['ethnonymEvidence'].items()]
    labels = {'missing_russian':'Нет русского перевода', 'unconfirmed_noun':'Именное значение не подтверждено переводом',
              'name_or_abbreviation':'Название организации / сокращение', 'foreign_column':'Попадание другой словарной колонки',
              'ocr':'Подозрительное распознавание текста', 'proper_name_sense':'Вероятное собственное имя в одном из значений',
              'meaning_part_of_speech':'Часть речи или смысл перевода', 'proper_name':'Вероятное собственное имя',
              'meaning_form':'Неверная словарная форма перевода', 'possible_ethnonym':'Возможный этноним без подтверждённого значения',
              'meaning_format':'Повреждённое оформление перевода', 'mixed_alphabet':'Латинские буквы внутри русского слова',
              'ethnonym_boundary':'Пограничный этноним / общая группа', 'inflected_headword':'Словоформа вместо начальной формы'}
    md += ['', 'ИСТӘК, СЕРКӘШ, ҺАРТАЙ и башкирские родовые названия сохранены. ТӨРКИ (тюрки/письменность), ҠЫПСАҠ, БЕЛЕШ, ТИПТӘР, КАМПУЧИ и ИНД оставлены для отдельного решения, см. ниже. Фильтр использует точный список заголовков, а не упоминания народов в пояснениях: БАСҠАҠ, БАТЫР, ТОЛПАР, СӘНСКӘК и т. п. не затронуты. ҠЫТАТ («китайка») сохранено: это название ткани, см. [словарь Ураксиной, т. 1, с. 461](https://tarat.ru/ru/targema/dictionary/uraksina-t1/461).', '',
           '## Кандидаты на следующую редактуру', '',
           'В первую очередь: ошибочная часть речи/форма перевода, OCR и имена. Наличие прилагательного в русском переводе не всегда делает слово недопустимым: «больной», «учёный», «столовая» могут быть существительными. Автоматического удаления по окончаниям нет.', '',
           table(['Категория', 'Число слов'], [(labels.get(k, k), v) for k, v in sorted(counts.items())]), '']
    priority = [x for x in flags if any(i['category'] != 'missing_russian' for i in x['issues'])]
    md += [table(['Слово', 'Сохранённые русские значения', 'Причина проверки'], [(x['word'], '; '.join(x['meanings']) or 'нет', ' '.join(i['reason'] for i in x['issues'] if i['category'] != 'missing_russian')) for x in priority]), '',
           '## Все слова без русского перевода', '',
           'Сейчас они допустимы как бонусы согласно действующему правилу проекта. Чтобы требовать перевод для каждого принимаемого слова, понадобится отдельное изменение политики. Пропуск перевода сам по себе не доказывает, что слово ошибочно.', '',
           ', '.join(x['word'] for x in flags if any(i['category'] == 'missing_russian' for i in x['issues'])), '',
           'Списки сходных написаний находятся в `near-translations-2026-11-12.md`. Полные записи удалений и источники кандидатов сохранены в соседних JSON. Старые миграционные архивы содержат прежние слова только для переноса сохранений и не являются активной игровой базой.', '',
           'Воспроизведение: `python scripts/audit-dictionary.py --baseline-ref v2.9.5`.', '']
    (report_dir / 'dictionary-review-2026-11-12.md').write_text('\n'.join(md), 'utf-8')
    print(json.dumps({'removed': len(removed), 'words': raw['wordCount'], 'targets': raw['targetWordCount'], 'nearPairs': dict(collections.Counter(p['difference'] for p in remaining_pairs)), 'flagged': len(flags), 'categories': counts}, ensure_ascii=False))


if __name__ == '__main__':
    main()
