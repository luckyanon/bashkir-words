const escape=value=>value.replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
function inline(text){
  return escape(text).replace(/\[([^\]]+)\]\((https:\/\/[^\s)]+)\)/g,'<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>');
}
/** Render the small, bundled attribution document without accepting raw HTML. */
export function renderAttribution(markdown){
  return markdown.trim().split(/\r?\n\s*\r?\n/).map(block=>{
    const lines=block.split(/\r?\n/);
    if(lines.every(line=>line.startsWith('- ')))return `<ul>${lines.map(line=>`<li>${inline(line.slice(2))}</li>`).join('')}</ul>`;
    if(block.startsWith('# '))return '';
    if(block.startsWith('## '))return `<h3>${inline(block.slice(3))}</h3>`;
    return `<p>${inline(lines.join(' '))}</p>`;
  }).join('');
}
