// themes.jsx — Color theme presets applied via CSS custom properties.
// Each theme overrides the root variable stack so EVERY chrome element
// re-skins automatically. Light/dark mode is set by isLight flag.

const THEMES = {
  alto: {
    label: 'Alto · Amber',
    isLight: false,
    swatch: ['#d99858', '#15120e', '#efe7d8', '#6fbf73', '#e25d5d'],
    vars: {} // empty = use stylesheet defaults
  },

  'alto-cream': {
    label: 'Alto · Cream',
    isLight: true,
    swatch: ['#c47834', '#ecebe7', '#1f1c16', '#2e8a3a', '#c43d3d'],
    vars: {
      '--bg-0': '#ecebe7',
      '--bg-1': '#f3f2ee',
      '--bg-2': '#e9e7e1',
      '--bg-3': '#ddd9d1',
      '--bg-4': '#cfcabe',
      '--bg-5': '#f6f4ee',
      '--line-1': '#d8d3c8',
      '--line-2': '#c4bdaf',
      '--line-3': '#a89f8d',
      '--ink-0': '#1f1c16',
      '--ink-1': '#3d3830',
      '--ink-2': '#6e6557',
      '--ink-3': '#948b7c',
      '--ink-4': '#b6ad9c',
      '--accent': '#c47834',
      '--accent-soft': '#c4783440',
      '--accent-glow': '#c4783430',
      '--accent-ink': '#8a4f1c',
      '--buy': '#2e8a3a',
      '--buy-soft': '#2e8a3a22',
      '--buy-ink': '#1c5a23',
      '--sell': '#c43d3d',
      '--sell-soft': '#c43d3d22',
      '--sell-ink': '#7a2222',
      '--hi-1': 'rgba(255,255,255,.55)',
      '--hi-2': 'rgba(255,255,255,.85)',
    }
  },

  newsprint: {
    label: 'Newsprint',
    isLight: true,
    swatch: ['#1a73e8', '#f5f3ec', '#1a1a1a', '#1b8043', '#c5221f'],
    vars: {
      '--bg-0': '#f7f5ee',
      '--bg-1': '#f0eee5',
      '--bg-2': '#e8e5d8',
      '--bg-3': '#dcd8c8',
      '--bg-4': '#c8c3b0',
      '--bg-5': '#f4f1e8',
      '--line-1': '#d4cfbe',
      '--line-2': '#b8b29c',
      '--line-3': '#8a8474',
      '--ink-0': '#1a1a1a',
      '--ink-1': '#3a3a36',
      '--ink-2': '#6a6a60',
      '--ink-3': '#8e8a7c',
      '--ink-4': '#b0ac9c',
      '--accent': '#1a73e8',
      '--accent-soft': '#1a73e828',
      '--accent-glow': '#1a73e820',
      '--accent-ink': '#0a4a9a',
      '--buy': '#1b8043',
      '--buy-soft': '#1b804320',
      '--buy-ink': '#0e5024',
      '--sell': '#c5221f',
      '--sell-soft': '#c5221f20',
      '--sell-ink': '#7a1614',
      '--hi-1': 'rgba(255,255,255,.55)',
      '--hi-2': 'rgba(255,255,255,.85)',
    }
  },

  monokai: {
    label: 'Monokai',
    isLight: false,
    swatch: ['#fd971f', '#272822', '#f8f8f2', '#a6e22e', '#f92672'],
    vars: {
      '--bg-0': '#1d1e19',
      '--bg-1': '#272822',
      '--bg-2': '#2d2e27',
      '--bg-3': '#34352d',
      '--bg-4': '#3e3f35',
      '--bg-5': '#49493f',
      '--line-1': '#34352d',
      '--line-2': '#49493f',
      '--line-3': '#5a5b4c',
      '--ink-0': '#f8f8f2',
      '--ink-1': '#cfd0c2',
      '--ink-2': '#9fa093',
      '--ink-3': '#75766a',
      '--ink-4': '#4a4b41',
      '--accent': '#fd971f',
      '--accent-soft': '#fd971f30',
      '--accent-glow': '#fd971f24',
      '--accent-ink': '#fdc684',
      '--buy': '#a6e22e',
      '--buy-soft': '#a6e22e22',
      '--buy-ink': '#d4ec8e',
      '--sell': '#f92672',
      '--sell-soft': '#f9267222',
      '--sell-ink': '#fc8db5',
      '--hi-1': 'rgba(248,248,242,.06)',
      '--hi-2': 'rgba(248,248,242,.10)',
    }
  },

  solarized: {
    label: 'Solarized · Light',
    isLight: true,
    swatch: ['#cb4b16', '#fdf6e3', '#586e75', '#859900', '#dc322f'],
    vars: {
      '--bg-0': '#fdf6e3',
      '--bg-1': '#f5eed4',
      '--bg-2': '#eee8d5',
      '--bg-3': '#e4dec3',
      '--bg-4': '#d6cfb0',
      '--bg-5': '#f5eed4',
      '--line-1': '#e4dec3',
      '--line-2': '#cdc6a8',
      '--line-3': '#a8a081',
      '--ink-0': '#073642',
      '--ink-1': '#586e75',
      '--ink-2': '#657b83',
      '--ink-3': '#839496',
      '--ink-4': '#bcb89a',
      '--accent': '#cb4b16',
      '--accent-soft': '#cb4b1640',
      '--accent-glow': '#cb4b1630',
      '--accent-ink': '#7a2b0e',
      '--buy': '#859900',
      '--buy-soft': '#85990022',
      '--buy-ink': '#536100',
      '--sell': '#dc322f',
      '--sell-soft': '#dc322f22',
      '--sell-ink': '#8c1f1d',
      '--hi-1': 'rgba(255,255,255,.55)',
      '--hi-2': 'rgba(255,255,255,.85)',
    }
  },

  'solarized-dark': {
    label: 'Solarized · Dark',
    isLight: false,
    swatch: ['#b58900', '#002b36', '#93a1a1', '#859900', '#dc322f'],
    vars: {
      '--bg-0': '#002b36',
      '--bg-1': '#073642',
      '--bg-2': '#0a3e4c',
      '--bg-3': '#114a59',
      '--bg-4': '#185366',
      '--bg-5': '#1e5f74',
      '--line-1': '#114a59',
      '--line-2': '#1e5f74',
      '--line-3': '#2a6e85',
      '--ink-0': '#fdf6e3',
      '--ink-1': '#eee8d5',
      '--ink-2': '#93a1a1',
      '--ink-3': '#657b83',
      '--ink-4': '#3a525a',
      '--accent': '#b58900',
      '--accent-soft': '#b5890030',
      '--accent-glow': '#b5890024',
      '--accent-ink': '#e3c266',
      '--buy': '#859900',
      '--buy-soft': '#85990022',
      '--buy-ink': '#b8d150',
      '--sell': '#dc322f',
      '--sell-soft': '#dc322f22',
      '--sell-ink': '#f08785',
      '--hi-1': 'rgba(253,246,227,.05)',
      '--hi-2': 'rgba(253,246,227,.10)',
    }
  },

  catppuccin: {
    label: 'Catppuccin · Mocha',
    isLight: false,
    swatch: ['#fab387', '#1e1e2e', '#cdd6f4', '#a6e3a1', '#f38ba8'],
    vars: {
      '--bg-0': '#181825',
      '--bg-1': '#1e1e2e',
      '--bg-2': '#252537',
      '--bg-3': '#2e2e44',
      '--bg-4': '#393952',
      '--bg-5': '#45456a',
      '--line-1': '#2e2e44',
      '--line-2': '#393952',
      '--line-3': '#45456a',
      '--ink-0': '#cdd6f4',
      '--ink-1': '#b4befe',
      '--ink-2': '#9399b2',
      '--ink-3': '#7f849c',
      '--ink-4': '#585b70',
      '--accent': '#fab387',
      '--accent-soft': '#fab38730',
      '--accent-glow': '#fab38724',
      '--accent-ink': '#fdc9aa',
      '--buy': '#a6e3a1',
      '--buy-soft': '#a6e3a122',
      '--buy-ink': '#c8edc4',
      '--sell': '#f38ba8',
      '--sell-soft': '#f38ba822',
      '--sell-ink': '#f7b3c5',
      '--hi-1': 'rgba(205,214,244,.06)',
      '--hi-2': 'rgba(205,214,244,.10)',
    }
  },

  dracula: {
    label: 'Dracula',
    isLight: false,
    swatch: ['#ff79c6', '#282a36', '#f8f8f2', '#50fa7b', '#ff5555'],
    vars: {
      '--bg-0': '#21222c',
      '--bg-1': '#282a36',
      '--bg-2': '#2f3140',
      '--bg-3': '#383a4a',
      '--bg-4': '#44475a',
      '--bg-5': '#525468',
      '--line-1': '#383a4a',
      '--line-2': '#44475a',
      '--line-3': '#5b5e76',
      '--ink-0': '#f8f8f2',
      '--ink-1': '#d8d8d0',
      '--ink-2': '#9a9ab0',
      '--ink-3': '#6c6e85',
      '--ink-4': '#44475a',
      '--accent': '#ff79c6',
      '--accent-soft': '#ff79c630',
      '--accent-glow': '#ff79c624',
      '--accent-ink': '#ffaadc',
      '--buy': '#50fa7b',
      '--buy-soft': '#50fa7b22',
      '--buy-ink': '#9affb4',
      '--sell': '#ff5555',
      '--sell-soft': '#ff555522',
      '--sell-ink': '#ff9999',
      '--hi-1': 'rgba(248,248,242,.06)',
      '--hi-2': 'rgba(248,248,242,.10)',
    }
  },

  nord: {
    label: 'Nord',
    isLight: false,
    swatch: ['#88c0d0', '#2e3440', '#eceff4', '#a3be8c', '#bf616a'],
    vars: {
      '--bg-0': '#242933',
      '--bg-1': '#2e3440',
      '--bg-2': '#353a47',
      '--bg-3': '#3b4252',
      '--bg-4': '#434c5e',
      '--bg-5': '#4c566a',
      '--line-1': '#3b4252',
      '--line-2': '#434c5e',
      '--line-3': '#4c566a',
      '--ink-0': '#eceff4',
      '--ink-1': '#d8dee9',
      '--ink-2': '#aeb4be',
      '--ink-3': '#7e8794',
      '--ink-4': '#566070',
      '--accent': '#88c0d0',
      '--accent-soft': '#88c0d030',
      '--accent-glow': '#88c0d024',
      '--accent-ink': '#b8dbe5',
      '--buy': '#a3be8c',
      '--buy-soft': '#a3be8c22',
      '--buy-ink': '#c6d6b3',
      '--sell': '#bf616a',
      '--sell-soft': '#bf616a22',
      '--sell-ink': '#d9969c',
      '--hi-1': 'rgba(236,239,244,.06)',
      '--hi-2': 'rgba(236,239,244,.10)',
    }
  },
};

function applyTheme(name){
  const t = THEMES[name] || THEMES.alto;
  const r = document.documentElement;

  // Toggle light/dark class
  r.classList.toggle('light', t.isLight);
  // Tag the theme so per-theme overrides can hook on
  r.setAttribute('data-theme', name);

  // Wipe any previously-set inline overrides from the last theme
  const allKeys = new Set();
  Object.values(THEMES).forEach(th => Object.keys(th.vars).forEach(k => allKeys.add(k)));
  allKeys.forEach(k => r.style.removeProperty(k));

  // Apply this theme's vars
  Object.entries(t.vars).forEach(([k, v]) => r.style.setProperty(k, v));
}

window.THEMES = THEMES;
window.applyTheme = applyTheme;
