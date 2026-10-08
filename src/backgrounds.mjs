export const BACKGROUNDS=Object.freeze([
 {id:'ufa',name:'Уфа',level:1,file:'./assets/background-ufa.webp'},
 {id:'salavat',name:'Салават',level:20,file:'./assets/background-salavat.webp'},
 {id:'baymak',name:'Баймак',level:40,file:'./assets/background-baymak.webp'},
 {id:'village',name:'Деревня',level:60,file:'./assets/background-village.webp'},
 {id:'olasay',name:'У бабушки',level:80,file:'./assets/background-olasay.webp'},
]);
export const availableBackground=(id,level)=>BACKGROUNDS.find(item=>item.id===id&&item.level<=level);
export const backgroundRewardsForLevel=id=>BACKGROUNDS.filter(item=>item.level>1&&item.level===id+1);

