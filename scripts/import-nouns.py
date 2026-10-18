"""Import only accepted nouns from the supplied ZIP; review files never enter gameplay."""
import pathlib,zipfile,json,sys,hashlib
root=pathlib.Path(__file__).resolve().parents[1]/'public'
with zipfile.ZipFile(sys.argv[1]) as z:
 full=json.loads(z.read('data/nouns.full.json'));raw=json.loads(z.read('data/game-lexicon.nouns.json'))
 full['entries']=[e for e in full['entries'] if e['display'] not in {'МИЛ','ТӨРӨК','АЛМАС'}]
 raw['words']=sorted(e['display'] for e in full['entries'])
 raw['targetWords']=sorted(e['display'] for e in full['entries'] if any(m.get('text','').strip() for m in e.get('definitions',{}).get('ru',[])))
 raw['datasetRevision']=hashlib.sha256(z.read('data/nouns.full.json')).hexdigest()[:16]
 raw.update(maxLength=max(map(len,raw['words'])),wordCount=len(raw['words']),targetWordCount=len(raw['targetWords']));raw.pop('index',None)
 (root/'data/game-lexicon.nouns.json').write_text(json.dumps(raw,ensure_ascii=False),encoding='utf-8')
 (root/'data/nouns.full.json').write_text(json.dumps(full,ensure_ascii=False),encoding='utf-8')
 (root/'data/nouns-sources.json').write_bytes(z.read('sources.json'))
 (root/'ATTRIBUTION.md').write_text(z.read('ATTRIBUTION.md').decode('utf-8').replace('\r\r\n','\n').replace('\r\n','\n'),encoding='utf-8')
print(json.dumps({'words':raw['wordCount'],'targets':raw['targetWordCount'],'maxLength':raw['maxLength']}))

