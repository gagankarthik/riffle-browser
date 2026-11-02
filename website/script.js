document.documentElement.classList.add('js');
const $ = id => document.getElementById(id);
const still = matchMedia('(prefers-reduced-motion: reduce)').matches;

// Entrances: the parts of each section arrive once, in reading order.
const rise = [...document.querySelectorAll('[data-rise]')];
const show = new IntersectionObserver(entries => {
  for (const e of entries) if (e.isIntersecting) { e.target.classList.add('in'); show.unobserve(e.target); }
}, {rootMargin: '0px 0px -8% 0px'});
rise.forEach(el => {
  const siblings = [...el.parentElement.querySelectorAll(':scope > [data-rise]')];
  el.style.setProperty('--delay', `${Math.min(siblings.indexOf(el), 3) * 80}ms`);
  show.observe(el);
});

const flip = button => {
  const on = button.getAttribute('aria-checked') !== 'true';
  button.setAttribute('aria-checked', String(on));
  return on;
};

// The first screen: Riffle's shield switch on an example page.
$('demo-switch').addEventListener('click', e => {
  const on = flip(e.currentTarget);
  $('demo').classList.toggle('ads-on', !on);
  $('demo-count').textContent = on ? '14 items blocked on this page' : 'Nothing blocked on this site';
  $('demo-detail').textContent = on ? '4 ads and 10 trackers, on this example page' : 'The ads are back, and the page moved to fit them';
  $('demo-hint').textContent = on ? 'Turn this off to see the page without Riffle.' : 'Turn it back on and the page closes up.';
});

// Tab sleeping, by the browser's own rule: the most recently used stay awake, and so does anything playing.
const names = ['River maps', 'Inbox', 'Rust docs', 'Radio', 'Train times', 'Soup recipes'];
const RADIO = 3;
let order = [0, 1, 4, RADIO, 2, 5];
let keep = 3;
let playing = true;
const tabs = names.map((name, i) => {
  const tab = document.createElement('button');
  tab.className = 'tab';
  tab.textContent = name;
  tab.addEventListener('click', () => {
    order = [i, ...order.filter(n => n !== i)];
    drawTabs();
  });
  $('sleep-list').append(tab);
  return tab;
});
function awakeSet() {
  const awake = new Set(order.slice(0, keep));
  if (playing) awake.add(RADIO);
  return awake;
}
function drawTabs() {
  const awake = awakeSet();
  tabs.forEach((tab, i) => {
    const on = awake.has(i), sound = i === RADIO && playing;
    tab.classList.toggle('asleep', !on);
    tab.setAttribute('aria-pressed', String(i === order[0]));
    tab.setAttribute('aria-label', `${names[i]}, ${on ? 'awake' : 'asleep'}${sound ? ', playing audio' : ''}`);
    tab.replaceChildren(names[i]);
    if (sound) tab.insertAdjacentHTML('beforeend', '<svg class="i" aria-hidden="true"><use href="#i-sound"></use></svg>');
    const state = document.createElement('small');
    state.textContent = on ? 'Awake' : 'Asleep';
    tab.append(state);
  });
  $('awake').textContent = keep;
  $('fewer').disabled = keep === 1;
  $('more').disabled = keep === names.length;
  const asleep = names.length - awake.size;
  $('sleep-state').textContent = asleep === 0 ? 'Every tab is awake.' : `${asleep} ${asleep === 1 ? 'tab has' : 'tabs have'} freed ${asleep === 1 ? 'its' : 'their'} memory.`;
}
$('fewer').addEventListener('click', () => { keep = Math.max(1, keep - 1); drawTabs(); });
$('more').addEventListener('click', () => { keep = Math.min(names.length, keep + 1); drawTabs(); });
$('music').addEventListener('click', e => { playing = flip(e.currentTarget); drawTabs(); });
drawTabs();

// Settings as sentences: each switch says what it does.
for (const band of document.querySelectorAll('.band')) {
  const button = band.querySelector('.switch'), result = band.querySelector('.band-result');
  const say = () => { result.textContent = button.getAttribute('aria-checked') === 'true' ? result.dataset.on : result.dataset.off; };
  say();
  button.addEventListener('click', () => { flip(button); say(); });
}

// Real screenshots, crossfading slowly. Stops when hidden, and stays on the first under reduced motion.
const fade = $('fade');
const pictures = [
  ['shots/tasks.webp', "Riffle's task manager listing tabs and processes with their memory."],
  ['shots/bookmark.webp', "Riffle's bookmark dialog with fields for the name and link."],
];
if (!still) {
  const top = fade.querySelector('img');
  const shown = [top, ...pictures.map(([src, alt]) => {
    const img = new Image(1600, 840);
    img.src = src;
    img.alt = alt;
    img.style.opacity = 0;
    img.setAttribute('aria-hidden', 'true');
    fade.append(img);
    return img;
  })];
  let at = 0;
  setInterval(() => {
    if (document.hidden) return;
    const next = (at + 1) % shown.length;
    shown[next].style.opacity = 1;
    shown[at].style.opacity = next === 0 ? 1 : 0;
    if (next === 0) shown.slice(1).forEach(img => { img.style.opacity = 0; });
    at = next;
  }, 6000);
}

// One question, one answer.
const questions = [...document.querySelectorAll('#faq-grid button')];
questions.forEach(button => button.addEventListener('click', () => {
  questions.forEach(b => b.setAttribute('aria-expanded', String(b === button)));
}));

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
