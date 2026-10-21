import { mount } from 'svelte'
import './app.css'
async function start() {
  const { default: Component } = new URLSearchParams(window.location.search).get('view') === 'tray'
    ? await import('./lib/TrayMenu.svelte')
    : await import('./App.svelte')
  return mount(Component, { target: document.getElementById('app')! })
}

const app = start()

export default app
