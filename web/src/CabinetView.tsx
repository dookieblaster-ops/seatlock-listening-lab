import { useEffect, useRef } from "react";
import * as THREE from "three";

type Props = {
  positions: Float32Array | null;
  indices: Uint32Array | null;
  driver: number[] | null;
  /** Further points to mark, such as the mounting points of an imported cabinet. */
  markers?: number[][];
  /** When given, a click on the cabinet reports the point and the surface's outward direction. */
  onPick?: (point: number[], normal: number[]) => void;
};

export function CabinetView({ positions, indices, driver, markers, onPick }: Props) {
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const parent = host.current;
    if (!parent || !positions || !indices) {
      return;
    }
    const height = 280;
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(parent.clientWidth, height);
    parent.replaceChildren(renderer.domElement);
    const scene = new THREE.Scene();
    scene.background = new THREE.Color("#f5f1ea");

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geometry.setIndex(new THREE.BufferAttribute(indices, 1));
    geometry.computeVertexNormals();
    geometry.computeBoundingBox();
    const box = geometry.boundingBox ?? new THREE.Box3(new THREE.Vector3(), new THREE.Vector3(0.16, 0.18, 0.25));
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3()).length();

    // Seen from in front, a little to the right and above, at a distance that fits the cabinet.
    const camera = new THREE.PerspectiveCamera(35, parent.clientWidth / height, size / 100, size * 20);
    camera.position.set(center.x + 0.45 * size, center.y - 1.6 * size, center.z + 0.7 * size);
    camera.lookAt(center);
    scene.add(new THREE.AmbientLight(0xffffff, 0.7));
    const light = new THREE.DirectionalLight(0xffffff, 1.2);
    light.position.set(-0.4, -0.6, 0.8);
    scene.add(light);
    const solid = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({ color: "#d7c4a3", roughness: 0.7 }));
    scene.add(solid);
    const dot = (p: number[], color: string) => {
      const marker = new THREE.Mesh(
        new THREE.SphereGeometry(size * 0.02, 16, 12),
        new THREE.MeshStandardMaterial({ color }),
      );
      marker.position.set(p[0], p[1], p[2]);
      scene.add(marker);
    };
    if (driver) {
      dot(driver, "#2f5d50");
    }
    (markers ?? []).forEach((p) => dot(p, "#9b3d2a"));
    renderer.render(scene, camera);

    const canvas = renderer.domElement;
    const click = (event: MouseEvent) => {
      if (!onPick) {
        return;
      }
      const rect = canvas.getBoundingClientRect();
      const pointer = new THREE.Vector2(
        ((event.clientX - rect.left) / rect.width) * 2 - 1,
        -((event.clientY - rect.top) / rect.height) * 2 + 1,
      );
      const ray = new THREE.Raycaster();
      ray.setFromCamera(pointer, camera);
      const hit = ray.intersectObject(solid)[0];
      if (!hit || !hit.face) {
        return;
      }
      const n = hit.face.normal;
      onPick([hit.point.x, hit.point.y, hit.point.z], [n.x, n.y, n.z]);
    };
    canvas.style.cursor = onPick ? "crosshair" : "default";
    canvas.addEventListener("click", click);
    return () => {
      canvas.removeEventListener("click", click);
      geometry.dispose();
      renderer.dispose();
    };
  }, [positions, indices, driver, markers, onPick]);

  return <div ref={host} data-testid="cabinet-view" style={{ minHeight: 280 }} />;
}
