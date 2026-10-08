export const BACKGROUNDS=Object.freeze([
 {id:'ufa',name:'Уфа',price:0,legacyLevel:1,file:'./assets/background-ufa.webp'},
 {id:'salavat',name:'Салават',price:1000,legacyLevel:20,file:'./assets/background-salavat.webp'},
 {id:'baymak',name:'Баймак',price:1600,legacyLevel:40,file:'./assets/background-baymak.webp'},
 {id:'village',name:'Деревня',price:2400,legacyLevel:60,file:'./assets/background-village.webp'},
 {id:'olasay',name:'У бабушки',price:3000,legacyLevel:80,file:'./assets/background-olasay.webp'},
]);
export const availableBackground=(id,owned=['ufa'])=>BACKGROUNDS.find(item=>item.id===id&&owned.includes(id));

