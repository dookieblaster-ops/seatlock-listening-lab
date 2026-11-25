import { useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";

export type StandingBand = {
  frequency_hz: number;
  re: number[];
  im: number[];
  max_abs: number;
};

export type PulseFrames = {
  dt_s: number;
  times_s: number[];
  frames: number[][];
  max_abs: number;
};

export type CapturedField = {
  dx_m: number;
  origin_m: number[];
  n: number[];
  cells: number[];
  positions_m: number[];
  standing: StandingBand[];
  pulse: PulseFrames | null;
};

export type WaveFieldReport = {
  domain: string;
  frequency_hz: number;
  field: CapturedField;
  notes: string[];
  elapsed_ms: number;
  markers_m: number[];
  marker_labels: string[];
};

type Mode = "standing" | "pulse";

type Props = {
  report: WaveFieldReport;
  cabinet?: { positions: Float32Array; indices: Uint32Array } | null;
};

/** Soft signed-log map so lobes and nodes stay readable when one hotspot dominates. */
export function signedLogMap(x: number, k = 9): number {
  const a = Math.abs(x);
  if (a < 1e-15) return 0;
  const mapped = Math.log1p(k * a) / Math.log1p(k);
  return Math.sign(x) * mapped;
}

/** Peak used for display: a high percentile, not the absolute max (avoids washed-out fields). */
export function robustPeak(values: ArrayLike<number>, percentile = 0.9): number {
  const abs: number[] = [];
  for (let i = 0; i < values.length; i++) {
    const v = Math.abs(values[i]);
    if (v > 1e-15) abs.push(v);
  }
  if (abs.length === 0) return 1e-12;
  abs.sort((a, b) => a - b);
  const idx = Math.min(abs.length - 1, Math.max(0, Math.floor(percentile * (abs.length - 1))));
  return Math.max(abs[idx], 1e-12);
}

/** Diverging pressure map: deep teal → near-black → amber (no purple). */
export function pressureColor(t: number, out: THREE.Color) {
  const x = Math.max(-1, Math.min(1, t));
  if (x < 0) {
    const u = -x;
    out.setRGB(0.04 + 0.05 * (1 - u), 0.22 + 0.7 * u, 0.28 + 0.55 * u);
  } else {
    out.setRGB(0.62 + 0.35 * x, 0.36 + 0.4 * x, 0.08 + 0.04 * (1 - x));
  }
  return out;
}

/** Phase that keeps a clear signed field when the user freezes the standing wave. */
export function snapshotPhase(phase: number): number {
  const step = Math.PI / 4;
  return Math.round(phase / step) * step;
}

const pointVertex = `
attribute float aScale;
varying vec3 vColor;
varying float vAlpha;
void main() {
  vColor = color;
  vAlpha = clamp(aScale * 0.55, 0.15, 1.0);
  vec4 mv = modelViewMatrix * vec4(position, 1.0);
  // Modest sizes — large additive points washed to white on software GL.
  float px = aScale * (90.0 / max(0.35, -mv.z));
  gl_PointSize = clamp(px, 1.5, 28.0);
  gl_Position = projectionMatrix * mv;
}
`;

const pointFragment = `
varying vec3 vColor;
varying float vAlpha;
void main() {
  vec2 p = gl_PointCoord * 2.0 - 1.0;
  float r = dot(p, p);
  if (r > 1.0) discard;
  float soft = exp(-2.6 * r);
  gl_FragColor = vec4(vColor, soft * vAlpha);
}
`;

export function WaveView({ report, cabinet }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const hasPulse = Boolean(report.field.pulse && report.field.pulse.frames.length > 1);
  const [mode, setMode] = useState<Mode>(hasPulse ? "pulse" : "standing");
  const [playing, setPlaying] = useState(true);
  const [threshold, setThreshold] = useState(report.domain === "outside" ? 0.18 : 0.12);
  const [scrub, setScrub] = useState(0);
  const modeRef = useRef(mode);
  const playingRef = useRef(playing);
  const thresholdRef = useRef(threshold);
  const scrubRef = useRef(scrub);
  const phaseRef = useRef(Math.PI / 4);
  const frameRef = useRef(0);
  const orbitPauseUntil = useRef(0);
  const userDragging = useRef(false);

  const band = report.field.standing[0];
  const pulseCount = report.field.pulse?.frames.length ?? 0;

  useEffect(() => {
    modeRef.current = mode;
  }, [mode]);
  useEffect(() => {
    playingRef.current = playing;
  }, [playing]);
  useEffect(() => {
    thresholdRef.current = threshold;
  }, [threshold]);
  useEffect(() => {
    scrubRef.current = scrub;
    if (mode === "pulse" && !playing) {
      frameRef.current = scrub;
    }
  }, [scrub, mode, playing]);

  useEffect(() => {
    if (!hasPulse && mode === "pulse") {
      setMode("standing");
    }
  }, [hasPulse, mode]);

  const title = useMemo(() => {
    if (report.domain === "inside") return "Sound inside the cabinet";
    if (report.domain === "outside") return "Sound around the cabinet";
    return "Bass waves in the room";
  }, [report.domain]);

  const subtitle = useMemo(() => {
    if (report.domain === "inside") return "Look into the box — pressure antinodes glow amber and teal";
    if (report.domain === "outside") return "How the cabinet shapes the field toward the listener";
    return "Assumed room shell from volume — walls as a wire frame";
  }, [report.domain]);

  useEffect(() => {
    const parent = host.current;
    if (!parent || !band) {
      return;
    }
    const width = parent.clientWidth || 640;
    const height = Math.max(420, Math.min(580, Math.round(width * 0.78)));
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(width, height);
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.localClippingEnabled = report.domain === "inside";
    parent.replaceChildren(renderer.domElement);

    const scene = new THREE.Scene();
    scene.background = new THREE.Color("#06090c");

    const positions = report.field.positions_m;
    const count = positions.length / 3;
    let cx = 0;
    let cy = 0;
    let cz = 0;
    let minX = Infinity;
    let minY = Infinity;
    let minZ = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    let maxZ = -Infinity;
    for (let i = 0; i < count; i++) {
      const x = positions[i * 3];
      const y = positions[i * 3 + 1];
      const z = positions[i * 3 + 2];
      cx += x;
      cy += y;
      cz += z;
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      minZ = Math.min(minZ, z);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
      maxZ = Math.max(maxZ, z);
    }
    cx /= Math.max(count, 1);
    cy /= Math.max(count, 1);
    cz /= Math.max(count, 1);
    const span = Math.max(maxX - minX, maxY - minY, maxZ - minZ, 0.2);

    const cutY = cy - 0.015 * span;
    const clipPlane =
      report.domain === "inside" ? new THREE.Plane(new THREE.Vector3(0, 1, 0), -cutY) : null;

    const camera = new THREE.PerspectiveCamera(38, width / height, span / 250, span * 60);
    scene.add(new THREE.AmbientLight(0xc5d0d8, 0.55));
    const key = new THREE.DirectionalLight(0xfff4e6, 1.1);
    key.position.set(cx - span, cy - span, cz + span);
    scene.add(key);
    const fill = new THREE.DirectionalLight(0x7ed6c3, 0.35);
    fill.position.set(cx + span, cy + 0.5 * span, cz - 0.3 * span);
    scene.add(fill);

    const disposables: { dispose: () => void }[] = [];

    const floor = new THREE.Mesh(
      new THREE.CircleGeometry(span * 1.55, 64),
      new THREE.MeshStandardMaterial({
        color: "#0a1016",
        roughness: 1,
        metalness: 0,
        transparent: true,
        opacity: 0.85,
      }),
    );
    floor.rotation.x = -Math.PI / 2;
    floor.position.set(cx, minY - 0.05 * span, cz);
    scene.add(floor);
    disposables.push(floor.geometry, floor.material as THREE.Material);

    if (cabinet && report.domain !== "room") {
      const geom = new THREE.BufferGeometry();
      geom.setAttribute("position", new THREE.BufferAttribute(cabinet.positions.slice(), 3));
      geom.setIndex(new THREE.BufferAttribute(cabinet.indices.slice(), 1));
      geom.computeVertexNormals();
      const shellMat = new THREE.MeshStandardMaterial({
        color: "#d8c9ae",
        roughness: 0.42,
        metalness: 0.14,
        transparent: true,
        opacity: report.domain === "inside" ? 0.42 : 0.3,
        side: THREE.DoubleSide,
        depthWrite: false,
        clippingPlanes: clipPlane ? [clipPlane] : [],
      });
      scene.add(new THREE.Mesh(geom, shellMat));
      const edgeMat = new THREE.LineBasicMaterial({
        color: "#f4ebd8",
        transparent: true,
        opacity: 0.7,
        clippingPlanes: clipPlane ? [clipPlane] : [],
      });
      scene.add(new THREE.LineSegments(new THREE.EdgesGeometry(geom, 18), edgeMat));
      disposables.push(geom, shellMat, edgeMat);

      if (report.domain === "inside" && clipPlane) {
        const ghostMat = new THREE.MeshBasicMaterial({
          color: "#8fa0ad",
          transparent: true,
          opacity: 0.05,
          side: THREE.DoubleSide,
          depthWrite: false,
          clippingPlanes: [new THREE.Plane(new THREE.Vector3(0, -1, 0), cutY)],
        });
        scene.add(new THREE.Mesh(geom, ghostMat));
        disposables.push(ghostMat);
      }
    }

    if (report.domain === "room") {
      const box = new THREE.BoxGeometry(maxX - minX || 1, maxY - minY || 1, maxZ - minZ || 1);
      const edgeMat = new THREE.LineBasicMaterial({ color: "#7a8d9c", transparent: true, opacity: 0.85 });
      const roomShell = new THREE.LineSegments(new THREE.EdgesGeometry(box), edgeMat);
      roomShell.position.set((minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2);
      scene.add(roomShell);
      disposables.push(box, edgeMat);
    }

    const cutMask = new Uint8Array(count);
    for (let i = 0; i < count; i++) {
      cutMask[i] = clipPlane && positions[i * 3 + 1] < cutY ? 0 : 1;
    }

    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.Float32BufferAttribute(Float32Array.from(positions), 3));
    const colors = new Float32Array(count * 3);
    const scales = new Float32Array(count);
    geom.setAttribute("color", new THREE.BufferAttribute(colors, 3));
    geom.setAttribute("aScale", new THREE.BufferAttribute(scales, 1));
    const material = new THREE.ShaderMaterial({
      vertexShader: pointVertex,
      fragmentShader: pointFragment,
      transparent: true,
      depthWrite: false,
      blending: THREE.NormalBlending,
      vertexColors: true,
    });
    scene.add(new THREE.Points(geom, material));
    disposables.push(geom, material);

    for (let i = 0; i < report.marker_labels.length; i++) {
      const mx = report.markers_m[i * 3];
      const my = report.markers_m[i * 3 + 1];
      const mz = report.markers_m[i * 3 + 2];
      if (mx === undefined) continue;
      const isListener = report.marker_labels[i] === "listener";
      const color = isListener ? "#f0c27a" : "#7ed6c3";
      const orb = new THREE.Mesh(
        new THREE.SphereGeometry(span * (isListener ? 0.03 : 0.026), 20, 16),
        new THREE.MeshStandardMaterial({
          color,
          emissive: color,
          emissiveIntensity: 0.4,
          roughness: 0.28,
          metalness: 0.2,
        }),
      );
      orb.position.set(mx, my, mz);
      scene.add(orb);
      const halo = new THREE.Mesh(
        new THREE.RingGeometry(span * 0.036, span * 0.05, 28),
        new THREE.MeshBasicMaterial({
          color,
          transparent: true,
          opacity: 0.65,
          side: THREE.DoubleSide,
          depthWrite: false,
        }),
      );
      halo.position.set(mx, my, mz);
      halo.lookAt(cx, cy - span, cz);
      scene.add(halo);
      disposables.push(orb.geometry, orb.material as THREE.Material, halo.geometry, halo.material as THREE.Material);
    }

    const scratch = new THREE.Color();
    const rawBuf = new Float32Array(count);
    // Point size in world-ish units; shader converts to pixels with a clamp.
    const baseSize = report.domain === "room" ? 2.8 : report.domain === "outside" ? 2.2 : 2.6;

    const paint = () => {
      const modeNow = modeRef.current;
      const thresh = thresholdRef.current;
      const breathe =
        modeNow === "standing" ? 0.88 + 0.12 * Math.abs(Math.cos(phaseRef.current)) : 1;
      const colorAttr = geom.getAttribute("color") as THREE.BufferAttribute;
      const scaleAttr = geom.getAttribute("aScale") as THREE.BufferAttribute;

      if (modeNow === "standing") {
        const cos = Math.cos(phaseRef.current);
        const sin = Math.sin(phaseRef.current);
        for (let i = 0; i < count; i++) {
          rawBuf[i] = band.re[i] * cos - band.im[i] * sin;
        }
      } else if (report.field.pulse && report.field.pulse.frames.length > 0) {
        const frames = report.field.pulse.frames;
        const index = Math.max(0, Math.min(Math.floor(frameRef.current || 0), frames.length - 1));
        const frame = frames[index];
        if (!frame || frame.length < count) {
          return;
        }
        for (let i = 0; i < count; i++) {
          rawBuf[i] = frame[i];
        }
      } else {
        for (let i = 0; i < count; i++) {
          rawBuf[i] = band.re[i];
        }
      }

      let sum = 0;
      let nSum = 0;
      for (let i = 0; i < count; i++) {
        if (!cutMask[i]) continue;
        sum += rawBuf[i];
        nSum += 1;
      }
      const mean = nSum > 0 ? sum / nSum : 0;
      for (let i = 0; i < count; i++) {
        rawBuf[i] -= mean;
      }
      const peak = Math.max(
        robustPeak(rawBuf, report.domain === "outside" ? 0.88 : 0.9),
        1e-12,
      );

      for (let i = 0; i < count; i++) {
        if (!cutMask[i]) {
          colorAttr.setXYZ(i, 0, 0, 0);
          scaleAttr.setX(i, 0);
          continue;
        }
        const value = signedLogMap(rawBuf[i] / peak);
        const mag = Math.abs(value);
        if (mag < thresh) {
          colorAttr.setXYZ(i, 0, 0, 0);
          scaleAttr.setX(i, 0);
          continue;
        }
        pressureColor(value, scratch);
        const lift = 0.9 + 0.45 * mag;
        colorAttr.setXYZ(i, scratch.r * lift, scratch.g * lift, scratch.b * lift);
        scaleAttr.setX(i, baseSize * (0.55 + 1.4 * mag) * breathe);
      }
      colorAttr.needsUpdate = true;
      scaleAttr.needsUpdate = true;
    };

    let yaw = 0.95;
    let pitch = 0.52;
    const targetYaw = 0.48;
    const targetPitch = 0.32;
    let introT = 0;
    const radius = report.domain === "room" ? 2.05 * span : 1.75 * span;
    const applyOrbit = () => {
      camera.position.set(
        cx + radius * Math.sin(yaw) * Math.cos(pitch),
        cy - radius * Math.sin(pitch),
        cz + radius * Math.cos(yaw) * Math.cos(pitch),
      );
      camera.lookAt(cx, cy, cz);
    };
    applyOrbit();

    const canvas = renderer.domElement;
    canvas.style.cursor = "grab";
    let dragging = false;
    let lastX = 0;
    let lastY = 0;
    const onDown = (event: PointerEvent) => {
      dragging = true;
      userDragging.current = true;
      orbitPauseUntil.current = performance.now() + 2800;
      lastX = event.clientX;
      lastY = event.clientY;
      canvas.style.cursor = "grabbing";
      canvas.setPointerCapture(event.pointerId);
    };
    const onMove = (event: PointerEvent) => {
      if (!dragging) return;
      yaw -= (event.clientX - lastX) * 0.005;
      pitch += (event.clientY - lastY) * 0.005;
      pitch = Math.max(-1.15, Math.min(1.15, pitch));
      lastX = event.clientX;
      lastY = event.clientY;
      introT = 1;
      applyOrbit();
    };
    const onUp = (event: PointerEvent) => {
      dragging = false;
      userDragging.current = false;
      orbitPauseUntil.current = performance.now() + 2800;
      canvas.style.cursor = "grab";
      try {
        canvas.releasePointerCapture(event.pointerId);
      } catch {
        /* ignore */
      }
    };
    canvas.addEventListener("pointerdown", onDown);
    canvas.addEventListener("pointermove", onMove);
    canvas.addEventListener("pointerup", onUp);

    let raf = 0;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;

      if (introT < 1) {
        introT = Math.min(1, introT + dt / 1.6);
        const ease = 1 - Math.pow(1 - introT, 3);
        yaw = 0.95 + (targetYaw - 0.95) * ease;
        pitch = 0.52 + (targetPitch - 0.52) * ease;
        applyOrbit();
      } else if (playingRef.current && !userDragging.current && now >= orbitPauseUntil.current) {
        yaw += dt * 0.16;
        applyOrbit();
      }

      if (playingRef.current) {
        if (modeRef.current === "standing") {
          phaseRef.current += dt * 1.55;
        } else {
          const n = report.field.pulse?.frames.length ?? 0;
          if (n > 0) {
            frameRef.current = (frameRef.current + dt * 12) % n;
            scrubRef.current = frameRef.current;
          }
        }
      }
      paint();
      renderer.render(scene, camera);
      raf = requestAnimationFrame(tick);
    };
    paint();
    raf = requestAnimationFrame(tick);

    return () => {
      cancelAnimationFrame(raf);
      canvas.removeEventListener("pointerdown", onDown);
      canvas.removeEventListener("pointermove", onMove);
      canvas.removeEventListener("pointerup", onUp);
      for (const item of disposables) {
        item.dispose();
      }
      renderer.dispose();
    };
  }, [report, cabinet, band, hasPulse]);

  const togglePlay = () => {
    setPlaying((p) => {
      if (p) {
        if (mode === "standing") {
          phaseRef.current = snapshotPhase(phaseRef.current);
        } else if (mode === "pulse") {
          setScrub(Math.floor(frameRef.current));
        }
      }
      return !p;
    });
  };

  return (
    <div className="wave-view" data-testid="wave-view">
      <div className="wave-view-head">
        <div>
          <h3>{title}</h3>
          <p className="wave-view-sub">{subtitle}</p>
        </div>
        <p className="wave-view-freq">{report.frequency_hz.toFixed(0)} Hz</p>
      </div>
      <div className="wave-legend" aria-hidden="true">
        <span className="wave-legend-teal">− pressure</span>
        <span className="wave-legend-bar" />
        <span className="wave-legend-amber">+ pressure</span>
      </div>
      <div ref={host} className="wave-view-canvas" data-testid="wave-view-canvas" />
      <div className="wave-view-controls row">
        <button
          type="button"
          data-testid="wave-mode-standing"
          className={mode === "standing" ? "active" : undefined}
          onClick={() => setMode("standing")}
        >
          Standing wave
        </button>
        <button
          type="button"
          data-testid="wave-mode-pulse"
          disabled={!hasPulse}
          className={mode === "pulse" ? "active" : undefined}
          onClick={() => hasPulse && setMode("pulse")}
          title={
            hasPulse
              ? "Slow-motion pressure pulse"
              : report.domain === "outside"
                ? "Outside uses the frequency-domain boundary-element model — standing wave only"
                : "No pulse for this domain"
          }
        >
          Pulse
        </button>
        <button type="button" data-testid="wave-play" onClick={togglePlay}>
          {playing ? "Pause" : "Play"}
        </button>
        {hasPulse && mode === "pulse" && (
          <>
            <button
              type="button"
              data-testid="wave-replay"
              onClick={() => {
                frameRef.current = 0;
                setScrub(0);
                setPlaying(true);
              }}
            >
              Replay
            </button>
            <label className="wave-slider">
              Time
              <input
                type="range"
                data-testid="wave-scrub"
                min={0}
                max={Math.max(pulseCount - 1, 0)}
                step={1}
                value={Math.min(scrub, Math.max(pulseCount - 1, 0))}
                onChange={(event) => {
                  const next = Number(event.target.value);
                  setScrub(next);
                  frameRef.current = next;
                  setPlaying(false);
                }}
              />
            </label>
          </>
        )}
        <label className="wave-slider">
          Hide quiet air
          <input
            type="range"
            min={0}
            max={0.55}
            step={0.01}
            value={threshold}
            onChange={(event) => setThreshold(Number(event.target.value))}
          />
        </label>
      </div>
      {report.notes.map((note) => (
        <p key={note} className="explain">
          {note}
        </p>
      ))}
      <p className="explain">
        Drag to turn — orbit pauses, then resumes. Teal is negative pressure, amber is positive.
        Inside views cut the front face so the cabinet stays the hero. Colors show the pattern
        relative to the spatial mean so nodes stay readable.
        {report.domain === "outside"
          ? " Outside has no pulse: the boundary-element solve is frequency-domain."
          : ""}
        {report.elapsed_ms > 0 ? ` Solved in ${(report.elapsed_ms / 1000).toFixed(1)} s.` : ""}
      </p>
    </div>
  );
}
