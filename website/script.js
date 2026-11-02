// Feature tour: real screenshots behind standard keyboard-friendly tabs.
const tabs = [...document.querySelectorAll('[role=tab]')];
const panel = document.getElementById('tour-panel');
const title = document.getElementById('tour-title');
const text = document.getElementById('tour-text');
const img = document.getElementById('tour-img');
let wanted = 0; // the latest choice, so a slow image can't replace a newer one

function choose(tab, focus) {
  for (const t of tabs) {
    const on = t === tab;
    t.setAttribute('aria-selected', String(on));
    t.tabIndex = on ? 0 : -1;
  }
  if (focus) tab.focus();
  panel.setAttribute('aria-labelledby', tab.id);
  title.textContent = tab.dataset.title;
  text.textContent = tab.dataset.text;
  const turn = ++wanted;
  img.classList.add('changing');
  const next = new Image();
  next.onload = next.onerror = () => {
    if (turn !== wanted) return;
    img.src = next.src;
    img.alt = tab.dataset.alt;
    img.classList.remove('changing');
  };
  next.src = `shots/${tab.dataset.shot}.webp`;
}

tabs.forEach((tab, i) => {
  tab.addEventListener('click', () => choose(tab));
  tab.addEventListener('keydown', e => {
    const step = {ArrowRight: 1, ArrowLeft: -1}[e.key];
    if (step) { e.preventDefault(); choose(tabs[(i + step + tabs.length) % tabs.length], true); }
    if (e.key === 'Home') { e.preventDefault(); choose(tabs[0], true); }
    if (e.key === 'End') { e.preventDefault(); choose(tabs[tabs.length - 1], true); }
  });
});

// Copy button, which says when it worked or that it could not.
for (const button of document.querySelectorAll('.copy')) {
  button.addEventListener('click', async () => {
    const label = button.textContent;
    try {
      await navigator.clipboard.writeText(button.dataset.copy);
      button.textContent = 'Copied';
    } catch {
      button.textContent = 'Press Ctrl+C to copy';
      getSelection().selectAllChildren(button.parentElement.querySelector('code'));
    }
    setTimeout(() => { button.textContent = label; }, 2000);
  });
}

// The real version and size, from the release itself. The text already on the page stays if this fails.
fetch('https://api.github.com/repos/gagankarthik/riffle-browser/releases/latest')
  .then(r => r.ok ? r.json() : Promise.reject(new Error(String(r.status))))
  .then(release => {
    const asset = release.assets.find(a => a.name === 'RiffleSetup.exe');
    if (!asset) return;
    const line = `Version ${release.tag_name.replace(/^v/, '')}, ${(asset.size / 1000000).toFixed(1)} MB. Free and open source. No administrator rights needed.`;
    for (const el of document.querySelectorAll('[data-release]')) el.textContent = line;
  })
  .catch(() => {});
