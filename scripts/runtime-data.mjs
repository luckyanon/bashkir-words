import {readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';

// Keep the full, attributed source in Git. Ship only fields actually used by
// gameplay and the translation/source dialog (no OCR/import metadata).
export async function writeRuntimeDictionary(out) {
  const full=JSON.parse(await readFile(new URL('../public/data/nouns.full.json',import.meta.url),'utf8'));
  const entries=full.entries.map(entry=>({id:entry.id,word:entry.word,display:entry.display,
    partsOfSpeech:entry.partsOfSpeech,
    definitions:{ru:(entry.definitions?.ru||[]).map(({text,partOfSpeech})=>({text,partOfSpeech}))},
    sources:(entry.sources||[]).map(({url,articleUrl,pageUrl})=>({url,articleUrl,pageUrl}))}));
  await writeFile(path.join(out,'data/nouns.full.json'),JSON.stringify({schemaVersion:full.schemaVersion,datasetRevision:full.datasetRevision,entries}));
}
