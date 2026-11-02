// The stream: horizontal waves that bend around each rock still present, and calm down once it's gone.
const canvas = document.getElementById('water');
const ctx = canvas.getContext('2d');
const still = matchMedia('(prefers-reduced-motion: reduce)').matches;
const rocks = [...document.querySelectorAll('.rock')].map(el => ({ el, x: 0, y: 0, r: 0, force: 1, on: true }));
const pointer = { x: 0, y: 0, r: 46, force: 0, on: false }; // the visitor's hand in the water
const clear = document.getElementById('clear');
const count = document.getElementById('count');
const LINE_GAP = 34, STEP = 8;
let width = 0, height = 0, running = false, visible = true;

function measure() {
  const scale = Math.min(devicePixelRatio || 1, 2);
  const box = canvas.getBoundingClientRect();
  width = box.width;
  height = box.height;
  canvas.width = width * scale;
  canvas.height = height * scale;
  ctx.setTransform(scale, 0, 0, scale, 0, 0);
  for (const rock of rocks) {
    const r = rock.el.getBoundingClientRect();
    rock.x = r.left + r.width / 2 - box.left;
    rock.y = r.top + r.height / 2 - box.top;
    rock.r = r.height / 2 + r.width / 5;
  }
}

function draw(t) {
  ctx.clearRect(0, 0, width, height);
  ctx.strokeStyle = 'rgba(255,255,255,.62)';
  ctx.lineWidth = 2.5;
  ctx.lineCap = 'round';
  for (let base = LINE_GAP / 2; base < height + LINE_GAP; base += LINE_GAP) {
    ctx.beginPath();
    for (let x = -STEP; x <= width + STEP; x += STEP) {
      let y = base + 5 * Math.sin(x * .011 - t * .9 + base * .05);
      for (const rock of [...rocks, pointer]) {
        if (rock.force < .01) continue;
        const across = base - rock.y, along = x - rock.x;
        const near = Math.exp(-Math.abs(across) / (rock.r * 1.5));
        y += Math.sign(across || 1) * rock.r * 1.1 * rock.force * near * Math.exp(-(along * along) / (2 * (rock.r * 1.6) ** 2));
        if (along > 0) y += 10 * rock.force * near * Math.exp(-along / 240) * Math.sin(along * .07 - t * 4); // the chop downstream
      }
      x === -STEP ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
    }
    ctx.stroke();
  }
}

function frame(ms) {
  if (!visible) { running = false; return; }
  for (const rock of [...rocks, pointer]) rock.force += ((rock.on ? 1 : 0) - rock.force) * .04;
  draw(ms / 1000);
  requestAnimationFrame(frame);
}

function start() {
  if (still) {
    for (const rock of rocks) rock.force = rock.on ? 1 : 0;
    draw(0);
  } else if (!running) {
    running = true;
    requestAnimationFrame(frame);
  }
}

function update() {
  const left = rocks.filter(r => r.on).length;
  rocks.forEach(r => { r.el.classList.toggle('gone', !r.on); r.el.tabIndex = r.on ? 0 : -1; });
  count.textContent = left === 5 ? '5 things in the way' : left ? `${5 - left} blocked, ${left} still in the way` : 'All 5 blocked. That\'s the idea.';
  clear.setAttribute('aria-pressed', String(left === 0));
  clear.textContent = left === 0 ? 'Turn off blocking' : 'Turn on blocking';
  start();
}

rocks.forEach(rock => rock.el.addEventListener('click', () => {
  rock.on = false;
  update();
  clear.focus({ preventScroll: true }); // the removed rock can't keep keyboard focus
}));
clear.addEventListener('click', () => {
  const on = rocks.some(r => r.on);
  rocks.forEach(r => { r.on = !on; });
  update();
});

const hero = document.getElementById('hero');
hero.addEventListener('pointermove', e => {
  const box = canvas.getBoundingClientRect();
  pointer.x = e.clientX - box.left;
  pointer.y = e.clientY - box.top;
  pointer.on = e.pointerType === 'mouse' && !still && pointer.y < box.height;
});
hero.addEventListener('pointerleave', () => { pointer.on = false; });

new ResizeObserver(() => { measure(); start(); }).observe(canvas);
new IntersectionObserver(([entry]) => { visible = entry.isIntersecting; start(); }).observe(canvas);
measure();
update();

// Tab sleeping demo, with the browser's own rule: the most recently used stay awake.
const AWAKE = 3;
const names = ['River maps', 'Inbox', 'Rust docs', 'Train times', 'Soup recipes', 'Weather'];
const strip = document.getElementById('strip');
const hint = document.getElementById('sleep-hint');
let order = [0, 1, 2]; // most recent first

const tabs = names.map((name, i) => {
  const tab = document.createElement('button');
  tab.className = 'tab';
  tab.textContent = name;
  tab.addEventListener('click', () => pick(i));
  strip.append(tab);
  return tab;
});

function render() {
  tabs.forEach((tab, i) => {
    const awake = order.includes(i);
    tab.classList.toggle('asleep', !awake);
    tab.setAttribute('aria-pressed', String(i === order[0]));
    tab.setAttribute('aria-label', `${names[i]}, ${awake ? 'awake' : 'asleep'}`);
  });
}

function pick(i) {
  const woke = !order.includes(i);
  order = [i, ...order.filter(n => n !== i)];
  const slept = order.length > AWAKE ? order.pop() : null;
  hint.textContent = woke ? `${names[i]} reloads. ${names[slept]} goes to sleep and frees its memory.` : `${names[i]} was already awake, so nothing reloads.`;
  render();
}
render();
