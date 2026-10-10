export function Stance() {
  return (
    <section data-section="stance" className="section stance" aria-labelledby="stance-title">
      <svg className="screen" viewBox="0 0 300 120" aria-hidden="true">
        <path className="marker marker--wall" d="M10 112 L36 18 L112 10 L112 104 Z" />
        <path className="marker marker--wall" d="M112 10 L188 10 L188 104 L112 104" />
        <path className="marker marker--wall" d="M188 10 L264 18 L290 112 L188 104" />
      </svg>
      <div className="stance-copy">
        <h2 id="stance-title" className="marker-head">
          A human runs the game.
        </h2>
        <p className="lede">
          ThunderForge will never build an AI game master. Tools can help a GM prepare, look things
          up and keep track. Nothing here will replace the person behind the screen. The people at
          the table are the point.
        </p>
      </div>
    </section>
  );
}
