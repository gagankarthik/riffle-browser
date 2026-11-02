// Reader mode: the page's main article alone, copied as plain text and pictures into a sealed layer. Run again to close.
(() => {
  const open = document.getElementById('riffle-reader');
  if (open) return open.remove();
  const clean = text => text.replace(/\s+/g, ' ').trim();
  const score = el => [...el.querySelectorAll('p')].reduce((sum, p) => sum + clean(p.textContent).length, 0);
  let best = document.querySelector('article, [role=main], main') || document.body;
  let top = score(best);
  for (const el of document.querySelectorAll('article, main, section, div')) {
    const s = score(el) - el.querySelectorAll('a').length * 8;
    if (s > top * 1.15 && s < top * 4 && el.offsetHeight) { best = el; top = s; }
  }
  const host = document.createElement('div');
  host.id = 'riffle-reader';
  host.style.cssText = 'all:initial;position:fixed;inset:0;z-index:2147483647';
  const root = host.attachShadow({mode: 'closed'});
  root.innerHTML = `<style>
    :host { color-scheme: light dark; }
    @media (prefers-reduced-motion: no-preference) { #page { animation: in .18s ease-out; } @keyframes in { from { opacity: 0; } } }
    #page { position: absolute; inset: 0; overflow-y: auto; background: light-dark(#f8f6f1, #1c1f22); color: light-dark(#1b2329, #e7eaec); font: 20px/1.6 Georgia, "Times New Roman", serif; }
    #page:focus { outline: none; }
    article { max-width: 680px; margin: 0 auto; padding: 56px 24px 96px; }
    h1, h2, h3 { font-family: "Segoe UI", sans-serif; line-height: 1.2; }
    h1 { font-size: 36px; margin: 0 0 24px; }
    img { display: block; max-width: 100%; height: auto; margin: 24px 0; }
    blockquote { margin: 24px 0; padding-left: 20px; border-left: 3px solid #0b6e8f; }
    pre { overflow-x: auto; font-size: 15px; }
    button { position: fixed; top: 16px; right: 24px; height: 36px; padding: 0 16px; border: 0; border-radius: 8px; background: #0b6e8f; color: #fff; font: 600 14px "Segoe UI", sans-serif; cursor: pointer; }
    button:hover { background: #085770; }
    button:focus-visible { outline: 3px solid currentColor; outline-offset: 2px; }
  </style><div id="page" tabindex="-1"><button>Close reader</button><article></article></div>`;
  const article = root.querySelector('article');
  const title = document.createElement('h1');
  title.textContent = clean(document.querySelector('h1')?.textContent || document.title);
  article.append(title);
  let words = 0;
  for (const el of best.querySelectorAll('p, h2, h3, h4, li, blockquote, pre, img')) {
    if (!el.offsetHeight && el.tagName !== 'IMG') continue;
    if (el.closest('nav, aside, footer, form, [role=navigation], [role=complementary]')) continue;
    if (el.parentElement.closest('li, blockquote')) continue;
    if (el.tagName === 'IMG') {
      if (/^https?:/.test(el.currentSrc || el.src) && el.naturalWidth > 200) {
        const img = document.createElement('img');
        img.src = el.currentSrc || el.src;
        img.alt = el.alt;
        article.append(img);
      }
      continue;
    }
    const text = el.tagName === 'PRE' ? el.textContent : clean(el.textContent);
    if (!text || (el.tagName === 'LI' && text.length < 30)) continue;
    const copy = document.createElement({H2: 'h2', H3: 'h3', H4: 'h3', BLOCKQUOTE: 'blockquote', PRE: 'pre'}[el.tagName] || 'p');
    copy.textContent = text; // text only, so nothing from the page can run in here
    article.append(copy);
    words += text.split(' ').length;
  }
  if (words < 80) {
    const note = document.createElement('p');
    note.textContent = "Riffle couldn't find an article on this page.";
    article.replaceChildren(note);
  }
  const close = () => { host.remove(); removeEventListener('keydown', esc, true); };
  const esc = e => { if (e.key === 'Escape') { e.stopPropagation(); close(); } };
  addEventListener('keydown', esc, true);
  root.querySelector('button').onclick = close;
  document.documentElement.append(host);
  root.getElementById('page').focus();
})();
