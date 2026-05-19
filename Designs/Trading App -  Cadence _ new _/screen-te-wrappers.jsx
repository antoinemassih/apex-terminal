// screen-dashboard-te.jsx & screen-terminal-te.jsx — TE wrappers
// Subtle refinement applied via the .te CSS layer:
//   • engraved keylines on panel headers
//   • disciplined accent (number chips become muted by default)
//   • deeper recessed wells / steppers / segments
//   • anodised knob treatment
//   • dot-matrix grille watermarks where they make sense

function ScreenDashboardTE(){
  return (
    <div className="te" style={{ height:'100%', display:'flex', flexDirection:'column' }}>
      <ScreenDashboard/>
    </div>
  );
}

function ScreenTerminalTE(){
  return (
    <div className="te" style={{ height:'100%', display:'flex', flexDirection:'column' }}>
      <ScreenTerminal/>
    </div>
  );
}

window.ScreenDashboardTE = ScreenDashboardTE;
window.ScreenTerminalTE = ScreenTerminalTE;
