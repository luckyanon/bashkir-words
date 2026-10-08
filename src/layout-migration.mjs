/** Freeze all previously unlocked boards; only future levels use compact placement. */
export function layoutOptionsForSave(saved,generatorVersion=6){
 if(!saved || saved.generatorVersion!==generatorVersion)return {layoutVersion:2,legacyLayoutThroughLevel:0};
 if(saved.layoutVersion===2)return {layoutVersion:2,legacyLayoutThroughLevel:Number.isSafeInteger(saved.legacyLayoutThroughLevel)&&saved.legacyLayoutThroughLevel>=0?saved.legacyLayoutThroughLevel:0};
 const candidates=[1,saved.currentLevel,saved.maxUnlocked,...(saved.completedRanges || []).map(range=>Number(range[1])+1)].filter(id=>Number.isSafeInteger(id)&&id>=1&&id<Number.MAX_SAFE_INTEGER);
 return {layoutVersion:2,legacyLayoutThroughLevel:candidates.reduce((max,id)=>Math.max(max,id),1)};
}
