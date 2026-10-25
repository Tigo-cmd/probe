import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';
import { applyTheme, getTheme } from './lib/theme.js';

// Before the first paint, so a forced theme never flashes the other one.
applyTheme(getTheme());

export default mount(App, { target: document.getElementById('app') });
