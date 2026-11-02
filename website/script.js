const $ = id => document.getElementById(id);

// The line between the page with ads and the page with Riffle.
$('compare-range').addEventListener('input', e => $('compare').style.setProperty('--pos', e.target.value + '%'));

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
