<script>
  // Line icons in the spirit of SF Symbols, drawn on a 24-unit grid with
  // round caps. Decorative: every use sits next to a text label.
  let { name, size = 20, stroke = 1.8 } = $props();

  const PATHS = {
    globe: 'M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18Z M3.6 9h16.8 M3.6 15h16.8 M12 3c-2.6 2.7-3.8 5.7-3.8 9s1.2 6.3 3.8 9 M12 3c2.6 2.7 3.8 5.7 3.8 9s-1.2 6.3-3.8 9',
    briefcase: 'M4 8h16a1 1 0 0 1 1 1v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V9a1 1 0 0 1 1-1Z M9 8V6a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2 M3 13h18',
    backpack: 'M8 6V5a4 4 0 0 1 8 0v1 M6 21h12a1 1 0 0 0 1-1v-9a5 5 0 0 0-5-5h-4a5 5 0 0 0-5 5v9a1 1 0 0 0 1 1Z M8 21v-5a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v5 M10 11h4',
    code: 'M8.5 7 3.5 12l5 5 M15.5 7l5 5-5 5 M13.5 4.5l-3 15',
    brush: 'M14.5 4.5 19.5 9.5 M12.6 6.4l5 5-6.3 6.3a2 2 0 0 1-1.4.6H7.5v-2.4a2 2 0 0 1 .6-1.4Z M4.5 21c1.8 0 3-1.2 3-3',
    film: 'M4 5h16a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1Z M7 5v14 M17 5v14 M3 9.5h4 M3 14.5h4 M17 9.5h4 M17 14.5h4',
    gamepad: 'M7.5 7h9a4.5 4.5 0 0 1 4.4 5.4l-.8 4.1a2.3 2.3 0 0 1-4 1L14.6 16H9.4l-1.5 1.5a2.3 2.3 0 0 1-4-1l-.8-4.1A4.5 4.5 0 0 1 7.5 7Z M8 10v4 M6 12h4 M15.5 11.5h.01 M17.5 13.5h.01',
    check: 'M5 12.5l4.5 4.5L19 7.5',
    caution: 'M12 4 2.8 19.5h18.4Z M12 10v4 M12 17h.01',
    cross: 'M6.5 6.5l11 11 M17.5 6.5l-11 11',
    question: 'M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18Z M9.6 9.3a2.5 2.5 0 0 1 4.8.9c0 1.7-2.4 2.2-2.4 3.8 M12 17h.01',
    chip: 'M7 7h10v10H7Z M10 10h4v4h-4Z M9.5 3v4 M14.5 3v4 M9.5 17v4 M14.5 17v4 M3 9.5h4 M3 14.5h4 M17 9.5h4 M17 14.5h4',
    memory: 'M3 8h18v8H3Z M6 11v2 M9 11v2 M12 11v2 M15 11v2 M18 11v2 M5 16v3 M19 16v3',
    drive: 'M4 6.5A1.5 1.5 0 0 1 5.5 5h13A1.5 1.5 0 0 1 20 6.5v11a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 17.5Z M4 14h16 M16.5 16.5h.01',
    graphics: 'M3 7h15a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H3Z M19 10h2 M19 14h2 M7.5 9.5a2.5 2.5 0 1 0 0 5a2.5 2.5 0 1 0 0-5Z M13 10h3 M13 14h3',
    display: 'M3.5 4.5h17a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-17a1 1 0 0 1-1-1v-10a1 1 0 0 1 1-1Z M9 20.5h6 M12 16.5v4',
    battery: 'M3.5 8h14a1.5 1.5 0 0 1 1.5 1.5v5a1.5 1.5 0 0 1-1.5 1.5h-14A1.5 1.5 0 0 1 2 14.5v-5A1.5 1.5 0 0 1 3.5 8Z M21.5 10.5v3',
    lock: 'M6 11h12a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1v-8a1 1 0 0 1 1-1Z M8 11V8a4 4 0 0 1 8 0v3',
    laptop: 'M5 5.5h14a1 1 0 0 1 1 1V16H4V6.5a1 1 0 0 1 1-1Z M2 16h20l-1 2.5H3Z',
    back: 'M14.5 5.5 8 12l6.5 6.5',
    compare: 'M7 4v13 M3.5 13.5 7 17l3.5-3.5 M17 20V7 M13.5 10.5 17 7l3.5 3.5',
    save: 'M12 4v11 M7.5 10.5 12 15l4.5-4.5 M4.5 15.5v3a1.5 1.5 0 0 0 1.5 1.5h12a1.5 1.5 0 0 0 1.5-1.5v-3',
    doc: 'M7 3h7l5 5v12a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1Z M14 3v5h5 M9 13h6 M9 17h6',
    folder: 'M3.5 6.5A1.5 1.5 0 0 1 5 5h4.2l2 2.2H19a1.5 1.5 0 0 1 1.5 1.5v9.3A1.5 1.5 0 0 1 19 19.5H5a1.5 1.5 0 0 1-1.5-1.5Z',
    shield: 'M12 3 4.5 6v5.5c0 4.6 3.1 8.2 7.5 9.5 4.4-1.3 7.5-4.9 7.5-9.5V6Z M8.8 12.2l2.2 2.2 4.3-4.6',
    eye: 'M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12Z M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6Z',
    sparkle: 'M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8Z M19 16l.8 2.2L22 19l-2.2.8L19 22l-.8-2.2L16 19l2.2-.8Z',
    chevron: 'M9.5 6l6 6-6 6',
    info: 'M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18Z M12 11v5.5 M12 7.5h.01',
  };
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <path d={PATHS[name] ?? PATHS.info} />
</svg>

<style>
  .icon {
    flex: none;
    display: block;
  }
</style>
