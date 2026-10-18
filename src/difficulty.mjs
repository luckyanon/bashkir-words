export const maxWordLengthForLevel=(id,dictionaryMax=12)=>Math.min(dictionaryMax,id<20?6:id<40?7:id<60?8:id<80?9:id<100?10:dictionaryMax);
export const minRackLengthForLevel=id=>id<80?3:4;
export const rackSizesForLevel=(id,dictionaryMax=12)=>Array.from({length:Math.max(0,maxWordLengthForLevel(id,dictionaryMax)-minRackLengthForLevel(id)+1)},(_,i)=>minRackLengthForLevel(id)+i);
// Hardcore keeps its previous progression.
export const hardcoreMaxWordLengthForLevel=(id,dictionaryMax=12)=>Math.min(dictionaryMax,id<20?8:id<50?9:id<70?10:id<100?11:12);
