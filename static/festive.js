// The countdown to Pakjesavond and the falling sweets. Decoration only: every
// form on the site works without it.

function updateCountdown() {
    const el = document.getElementById('timer');
    if (!el) return;
    const now = new Date();
    const year = now.getFullYear();
    let target = new Date(year, 11, 5, 18, 0, 0); // 5 December, 18:00
    if (now > target) {
        if (now.getDate() === 5 && now.getMonth() === 11) {
            document.getElementById('countdown').textContent = '🎁 Fijne Pakjesavond! 🎁';
            return;
        }
        target.setFullYear(year + 1);
    }
    const diff = target - now;
    const days = Math.floor(diff / 86400000);
    const hours = Math.floor((diff % 86400000) / 3600000);
    const minutes = Math.floor((diff % 3600000) / 60000);
    const seconds = Math.floor((diff % 60000) / 1000);
    el.textContent = `${days}d ${hours}u ${minutes}m ${seconds}s`;
}
setInterval(updateCountdown, 1000);
updateCountdown();

function createRain() {
    const container = document.getElementById('rain');
    if (!container) return;
    const items = ['🍪', '🍊', '🎁', '🍬', '🍫', '🧸'];
    const el = document.createElement('div');
    el.classList.add('falling-item');
    el.textContent = items[Math.floor(Math.random() * items.length)];
    el.style.left = Math.random() * 100 + 'vw';
    el.style.animationDuration = (Math.random() * 3 + 2) + 's';
    el.style.fontSize = (Math.random() * 20 + 20) + 'px';
    container.appendChild(el);
    setTimeout(() => el.remove(), 5000);
}
setInterval(createRain, 300);
