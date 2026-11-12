"""Apply curated exclusions/additions to both current data and future imports."""
import hashlib
import json
import pathlib
import re

POLICY = json.loads(pathlib.Path(__file__).with_name('dictionary-policy.json').read_text('utf-8'))
EXCLUDED = (set(POLICY['excludedWords']) | set(POLICY['abbreviations']) |
            set(POLICY.get('excludedEthnonyms', {})))
ABBREVIATION_TAGS = {'abbreviation', 'acronym', 'initialism'}


def is_abbreviation(entry):
    if entry['display'].upper() in POLICY['abbreviations']:
        return True
    # Match explicit usage labels, not ordinary translations such as
    # «сокращение длины» or the noun «ҡыҫҡартма» (abbreviation).
    senses = [sense for senses in entry.get('definitions', {}).values() for sense in senses]
    return any(ABBREVIATION_TAGS.intersection(sense.get('tags', [])) or
               re.search(r'\((?:сокращение|сокр\.|аббревиатура)\)|^(?:сокр\.|аббр\.|abbreviation of\b|short for\b)',
                         sense.get('text', ''), re.I) for sense in senses)


def apply_policy(raw, full):
    entries = [entry for entry in full['entries']
               if entry['display'].upper() not in EXCLUDED and not is_abbreviation(entry)]
    present = {entry['display'] for entry in entries}
    entries.extend(entry for entry in POLICY['additions'] if entry['display'] not in present)
    full['entries'] = entries
    raw['words'] = sorted(entry['display'] for entry in entries)
    raw['targetWords'] = sorted(entry['display'] for entry in entries
                          if any(sense.get('text', '').strip() for sense in entry.get('definitions', {}).get('ru', [])))
    raw['starterWords'] = [word for word in raw.get('starterWords', []) if word in raw['targetWords']]
    # Content-addressed revision keeps a later import from silently reusing a
    # revision belonging to different boards.
    revision = hashlib.sha256(json.dumps(entries, ensure_ascii=False, sort_keys=True).encode()).hexdigest()[:16]
    raw.update(datasetRevision=revision, maxLength=max(map(len, raw['words'])), wordCount=len(entries),
               targetWordCount=len(raw['targetWords']), starterWordCount=len(raw['starterWords']))
    raw.pop('index', None)
    full.update(datasetRevision=revision, entryCount=len(entries))
    for data in (raw, full):
        data['editorialPolicy'] = {'revision': POLICY['revision'], 'excludedWords': sorted(EXCLUDED),
                                   'addedWords': [entry['display'] for entry in POLICY['additions']]}
    return raw, full
