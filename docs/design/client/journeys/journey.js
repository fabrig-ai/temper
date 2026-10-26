'use strict';

function journeyNote() {
  const el = document.createElement('div');
  el.textContent = 'The journey goes no further here.';
  el.style.cssText = 'position:fixed;z-index:30;right:22px;bottom:22px;max-width:340px;padding:11px 14px;border-radius:10px;background:#302d28;color:#fff;box-shadow:0 10px 30px rgba(0,0,0,.16);font-size:12px';
  document.body.append(el);
  setTimeout(() => el.remove(), 2600);
}

document.addEventListener('click', event => {
  if (!(event.target instanceof Element)) return;
  const next = event.target.closest('[data-journey-next]:not([data-journey-act])');
  if (next) {
    event.preventDefault();
    location.href = next.dataset.journeyNext;
  } else if (event.target.closest('button, a[href]')) {
    const button = event.target.closest('button');
    if (button && button.type === 'submit' && button.form) return;
    event.preventDefault();
    journeyNote();
  }
});

document.addEventListener('submit', event => {
  event.preventDefault();
  if (!(event.target instanceof HTMLFormElement)) return;
  const form = event.target;
  const selector = '[data-journey-act="submit"][data-journey-next]';
  const next = form.matches(selector) ? form : Array.from(form.elements).find(field => field.matches(selector));
  if (next) location.href = next.dataset.journeyNext;
  else journeyNote();
});

document.addEventListener('change', event => {
  if (!(event.target instanceof Element)) return;
  const next = event.target.closest('[data-journey-act="toggle"][data-journey-next]');
  if (next) location.href = next.dataset.journeyNext;
});

document.addEventListener('keydown', event => {
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
  if (event.target instanceof Element && event.target.closest('input, textarea, select, [contenteditable]')) return;
  const rel = event.key === 'p' ? 'prev' : event.key === 'n' ? 'next' : null;
  if (!rel) return;
  const link = document.head.querySelector(`link[rel="${rel}"]`);
  if (link) {
    event.preventDefault();
    location.href = link.href;
  }
});
