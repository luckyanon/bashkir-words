export const maxWordLengthForLevel=(id,dictionaryMax=12)=>Math.min(dictionaryMax,id<20?8:id<50?9:id<70?10:id<100?11:12);
