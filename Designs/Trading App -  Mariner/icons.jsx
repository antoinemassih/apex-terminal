// icons.jsx — small set of phosphor-flavoured stroke icons (1.5px stroke, 16x16)

const Ic = ({ d, size = 14, stroke = 1.5, fill = 'none', children, viewBox = "0 0 16 16" }) => (
  <svg width={size} height={size} viewBox={viewBox} fill={fill} stroke="currentColor"
       strokeWidth={stroke} strokeLinecap="round" strokeLinejoin="round" style={{ display:'block', flex:'none' }}>
    {d ? <path d={d}/> : children}
  </svg>
);

const Icon = {
  Search:  (p) => <Ic {...p}><circle cx="7" cy="7" r="4.5"/><path d="M14 14l-3.6-3.6"/></Ic>,
  Plus:    (p) => <Ic {...p} d="M8 3v10M3 8h10"/>,
  Minus:   (p) => <Ic {...p} d="M3 8h10"/>,
  Close:   (p) => <Ic {...p} d="M4 4l8 8M12 4l-8 8"/>,
  Caret:   (p) => <Ic {...p} d="M4 6l4 4 4-4"/>,
  CaretL:  (p) => <Ic {...p} d="M10 4l-4 4 4 4"/>,
  CaretR:  (p) => <Ic {...p} d="M6 4l4 4-4 4"/>,
  Bell:    (p) => <Ic {...p} d="M4 11.5h8M6.5 11.5v.7a1.5 1.5 0 003 0v-.7M5 11.5V8a3 3 0 016 0v3.5"/>,
  Gear:    (p) => <Ic {...p}><circle cx="8" cy="8" r="2"/><path d="M8 1.5v1.7M8 12.8v1.7M14.5 8h-1.7M3.2 8H1.5M12.6 3.4l-1.2 1.2M4.6 11.4l-1.2 1.2M12.6 12.6l-1.2-1.2M4.6 4.6L3.4 3.4"/></Ic>,
  Maximize:(p) => <Ic {...p} d="M3 6V3h3M13 6V3h-3M3 10v3h3M13 10v3h-3"/>,
  Min:     (p) => <Ic {...p} d="M3 8h10"/>,
  Dot:     (p) => <Ic {...p}><circle cx="8" cy="8" r="2.2" fill="currentColor"/></Ic>,
  Up:      (p) => <Ic {...p} d="M3 11l5-6 5 6"/>,
  Down:    (p) => <Ic {...p} d="M3 5l5 6 5-6"/>,
  Chart:   (p) => <Ic {...p} d="M2 13h12M4 11V8M7 11V5M10 11V7M13 11V3"/>,
  Candle:  (p) => <Ic {...p}><path d="M4 3v10M4 5h0M4 11h0M8 2v12M8 5v6M12 4v8M12 6h0M12 10h0"/><rect x="3" y="5" width="2" height="6"/><rect x="7" y="5" width="2" height="6"/><rect x="11" y="6" width="2" height="4"/></Ic>,
  Layers:  (p) => <Ic {...p} d="M8 2L2 5l6 3 6-3-6-3zM2 9l6 3 6-3M2 12l6 3 6-3"/>,
  Pen:     (p) => <Ic {...p} d="M11.5 2.5l2 2L6 12l-3 1 1-3 7.5-7.5z"/>,
  Trash:   (p) => <Ic {...p} d="M3 4h10M5 4V2.5h6V4M5 4v9h6V4M7 6v5M9 6v5"/>,
  Eye:     (p) => <Ic {...p}><path d="M1 8s2.5-4.5 7-4.5S15 8 15 8s-2.5 4.5-7 4.5S1 8 1 8z"/><circle cx="8" cy="8" r="2"/></Ic>,
  Filter:  (p) => <Ic {...p} d="M2 3h12l-4.5 6v4l-3 1.5V9L2 3z"/>,
  Lock:    (p) => <Ic {...p} d="M4 8h8v6H4zM5.5 8V5.5a2.5 2.5 0 015 0V8"/>,
  News:    (p) => <Ic {...p} d="M2 3h11v10H2zM5 6h5M5 8h5M5 10h3"/>,
  Wave:    (p) => <Ic {...p} d="M1 8c2 0 2-4 4-4s2 8 4 8 2-8 4-8 2 4 2 4"/>,
  Func:    (p) => <Ic {...p} d="M3 13c0-3 2-3 4-3s2-3 2-5"/>,
  ArrowR:  (p) => <Ic {...p} d="M3 8h10M9 4l4 4-4 4"/>,
  ArrowL:  (p) => <Ic {...p} d="M13 8H3M7 4L3 8l4 4"/>,
  Drag:    (p) => <Ic {...p}><circle cx="6" cy="4" r=".8" fill="currentColor"/><circle cx="10" cy="4" r=".8" fill="currentColor"/><circle cx="6" cy="8" r=".8" fill="currentColor"/><circle cx="10" cy="8" r=".8" fill="currentColor"/><circle cx="6" cy="12" r=".8" fill="currentColor"/><circle cx="10" cy="12" r=".8" fill="currentColor"/></Ic>,
  Book:    (p) => <Ic {...p} d="M2 3h5l1 1 1-1h5v10H9l-1-1-1 1H2V3zM8 4v9"/>,
  Heat:    (p) => <Ic {...p} d="M2 2h6v6H2zM8 2h6v6H8zM2 8h6v6H2zM8 8h6v6H8z"/>,
  Pulse:   (p) => <Ic {...p} d="M1 8h3l2-5 4 10 2-5h3"/>,
  Logo:    (p) => <Ic {...p} viewBox="0 0 20 20" stroke={1.4} d="M3 16L10 4l7 12M6 12h8"/>,
};

window.Icon = Icon;
