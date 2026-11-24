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

/** Diverging pressure map: deep teal → ink → amber. */
function pressureColor(t: number, out: THREE.Color) {
  const x = Math.max(-1, Math.min(1, t));
  if (x < 0) {
    const u = -x;
    out.setRGB(0.04 + 0.08 * (1 - u), 0.12 + 0.55 * u, 0.18 + 0.55 * u);
  } else {
    out.setRGB(0.55 + 0.4 * x, 0.28 + 0.35 * x, 0.08 + 0.05 * (1 - x));
  }
  return out;
}

const pointVertex = `
attribute float aScale;
varying vec3 vColor;
void main() {
  vColor = color;
  vec4 mv = modelViewMatrix * vec4(position, 1.0);
  gl_PointSize = aScale * (280.0 / max(0.1, -mv.z));
  gl_Position = projectionMatrix * mv;
}
`;

const pointFragment = `
varying vec3 vColor;
void main() {
  vec2 p = gl_PointCoord * 2.0 - 1.0;
  float r = dot(p, p);
  if (r > 1.0) discard;
  float soft = exp(-2.8 * r);
  gl_FragColor = vec4(vColor, soft);
}
`;

export function WaveView({ report, cabinet }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const hasPulse = Boolean(report.field.pulse && report.field.pulse.frames.length > 1);
  const [mode, setMode] = useState<Mode>(hasPulse ? "pulse" : "standing");
  const [playing, setPlaying] = useState(true);
  const [threshold, setThreshold] = useState(0.12);
  const modeRef = useRef(mode);
  const playingRef = useRef(playing);
  const thresholdRef = useRef(threshold);
  const phaseRef = useRef(0);
  const frameRef = useRef(0);

  const band = report.field.standing[0];

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
    if (!hasPulse && mode === "pulse") {
      setMode("standing");
    }
  }, [hasPulse, mode]);

  const title = useMemo(() => {
    if (report.domain === "inside") return "Sound inside the cabinet";
    if (report.domain === "outside") return "Sound around the cabinet";
    return "Bass waves in the room";
  }, [report.domain]);

  useEffect(() => {
    const parent = host.current;
    if (!parent || !band) {
      return;
    }
    const width = parent.clientWidth || 640;
    const height = Math.max(380, Math.min(540, Math.round(width * 0.72)));
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(width, height);
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    parent.replaceChildren(renderer.domElement);

    const scene = new THREE.Scene();
    scene.background = new THREE.Color("#0b1014");
    scene.fog = new THREE.FogExp2(0x0b1014, report.domain === "room" ? 0.04 : 0.07);

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

    const camera = new THREE.PerspectiveCamera(42, width / height, span / 200, span * 40);
    scene.add(new THREE.AmbientLight(0xb8c4ce, 0.55));
    const key = new THREE.DirectionalLight(0xfff2e0, 1.15);
    key.position.set(cx - span, cy - span, cz + span);
    scene.add(key);
    const fill = new THREE.DirectionalLight(0x7ec8c3, 0.4);
    fill.position.set(cx + span, cy + 0.4 * span, cz - 0.2 * span);
    scene.add(fill);

    const ground = new THREE.Mesh(
      new THREE.PlaneGeometry(span * 4, span * 4),
      new THREE.MeshStandardMaterial({
        color: "#12181e",
        roughness: 0.95,
        metalness: 0.05,
        transparent: true,
        opacity: 0.9,
      }),
    );
    ground.rotation.x = -Math.PI / 2;
    ground.position.set(cx, minY - 0.03 * span, cz);
    scene.add(ground);

    const disposables: { dispose: () => void }[] = [ground.geometry, ground.material as THREE.Material];

    if (cabinet && report.domain !== "room") {
      const geom = new THREE.BufferGeometry();
      geom.setAttribute("position", new THREE.BufferAttribute(cabinet.positions.slice(), 3));
      geom.setIndex(new THREE.BufferAttribute(cabinet.indices.slice(), 1));
      geom.computeVertexNormals();
      const shellMat = new THREE.MeshStandardMaterial({
        color: "#c4b49a",
        roughness: 0.55,
        metalness: 0.08,
        transparent: true,
        opacity: report.domain === "inside" ? 0.16 : 0.3,
        side: THREE.DoubleSide,
        depthWrite: false,
      });
      scene.add(new THREE.Mesh(geom, shellMat));
      const edgeMat = new THREE.LineBasicMaterial({ color: "#e8dcc8", transparent: true, opacity: 0.32 });
      scene.add(new THREE.LineSegments(new THREE.EdgesGeometry(geom, 28), edgeMat));
      disposables.push(geom, shellMat, edgeMat);
    }

    if (report.domain === "room") {
      const box = new THREE.BoxGeometry(maxX - minX || 1, maxY - minY || 1, maxZ - minZ || 1);
      const edgeMat = new THREE.LineBasicMaterial({ color: "#5a6a78", transparent: true, opacity: 0.55 });
      const roomShell = new THREE.LineSegments(new THREE.EdgesGeometry(box), edgeMat);
      roomShell.position.set((minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2);
      scene.add(roomShell);
      disposables.push(box, edgeMat);
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
      blending: THREE.AdditiveBlending,
      vertexColors: true,
    });
    scene.add(new THREE.Points(geom, material));
    disposables.push(geom, material);

    for (let i = 0; i < report.marker_labels.length; i++) {
      const mx = report.markers_m[i * 3];
      const my = report.markers_m[i * 3 + 1];
      const mz = report.markers_m[i * 3 + 2];
      if (mx === undefined) continue;
      const color = report.marker_labels[i] === "listener" ? "#f0c27a" : "#7ed6c3";
      const orb = new THREE.Mesh(
        new THREE.SphereGeometry(span * 0.022, 20, 16),
        new THREE.MeshStandardMaterial({
          color,
          emissive: color,
          emissiveIntensity: 0.5,
          roughness: 0.35,
        }),
      );
      orb.position.set(mx, my, mz);
      scene.add(orb);
      disposables.push(orb.geometry, orb.material as THREE.Material);
    }

    const scratch = new THREE.Color();
    const baseSize = Math.max(4, report.field.dx_m * (report.domain === "room" ? 90 : 140));
    const paint = () => {
      const modeNow = modeRef.current;
      const thresh = thresholdRef.current;
      const colorAttr = geom.getAttribute("color") as THREE.BufferAttribute;
      const scaleAttr = geom.getAttribute("aScale") as THREE.BufferAttribute;
      if (modeNow === "standing") {
        const peak = Math.max(band.max_abs, 1e-12);
        const cos = Math.cos(phaseRef.current);
        const sin = Math.sin(phaseRef.current);
        for (let i = 0; i < count; i++) {
          const value = (band.re[i] * cos - band.im[i] * sin) / peak;
          const mag = Math.abs(value);
          if (mag < thresh) {
            colorAttr.setXYZ(i, 0, 0, 0);
            scaleAttr.setX(i, 0);
            continue;
          }
          pressureColor(value, scratch);
          colorAttr.setXYZ(i, scratch.r, scratch.g, scratch.b);
          scaleAttr.setX(i, baseSize * (0.35 + 1.4 * mag));
        }
      } else if (report.field.pulse && report.field.pulse.frames.length > 0) {
        const frames = report.field.pulse.frames;
        const index = Math.max(
          0,
          Math.min(Math.floor(frameRef.current || 0), frames.length - 1),
        );
        const frame = frames[index];
        if (!frame || frame.length < count) {
          return;
        }
        const peak = Math.max(report.field.pulse.max_abs, 1e-12);
        for (let i = 0; i < count; i++) {
          const value = frame[i] / peak;
          const mag = Math.abs(value);
          if (mag < thresh) {
            colorAttr.setXYZ(i, 0, 0, 0);
            scaleAttr.setX(i, 0);
            continue;
          }
          pressureColor(value, scratch);
          colorAttr.setXYZ(i, scratch.r, scratch.g, scratch.b);
          scaleAttr.setX(i, baseSize * (0.3 + 1.6 * mag));
        }
      } else {
        // No usable pulse: paint the standing band so the canvas is never blank.
        const peak = Math.max(band.max_abs, 1e-12);
        for (let i = 0; i < count; i++) {
          const value = band.re[i] / peak;
          const mag = Math.abs(value);
          if (mag < thresh) {
            colorAttr.setXYZ(i, 0, 0, 0);
            scaleAttr.setX(i, 0);
            continue;
          }
          pressureColor(value, scratch);
          colorAttr.setXYZ(i, scratch.r, scratch.g, scratch.b);
          scaleAttr.setX(i, baseSize * (0.35 + 1.4 * mag));
        }
      }
      colorAttr.needsUpdate = true;
      scaleAttr.needsUpdate = true;
    };

    let yaw = 0.4;
    let pitch = 0.38;
    const radius = 1.9 * span;
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
      applyOrbit();
    };
    const onUp = (event: PointerEvent) => {
      dragging = false;
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
      if (playingRef.current) {
        if (modeRef.current === "standing") {
          phaseRef.current += dt * 1.85;
        } else {
          const n = report.field.pulse?.frames.length ?? 0;
          if (n > 0) {
            frameRef.current = (frameRef.current + dt * 16) % n;
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

  return (
    <div className="wave-view" data-testid="wave-view">
      <div className="wave-view-head">
        <h3>{title}</h3>
        <p className="wave-view-freq">{report.frequency_hz.toFixed(0)} Hz</p>
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
          title={hasPulse ? "Slow-motion pressure pulse" : "No pulse for this domain"}
        >
          Pulse
        </button>
        <button type="button" data-testid="wave-play" onClick={() => setPlaying((p) => !p)}>
          {playing ? "Pause" : "Play"}
        </button>
        <label className="wave-slider">
          Hide quiet air
          <input
            type="range"
            min={0}
            max={0.45}
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
        Drag to turn the view. Teal is negative pressure, amber is positive.
        {report.elapsed_ms > 0 ? ` Solved in ${(report.elapsed_ms / 1000).toFixed(1)} s.` : ""}
      </p>
    </div>
  );
}
