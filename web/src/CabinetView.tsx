import { useEffect, useRef } from "react";
import * as THREE from "three";

type Props = {
  positions: Float32Array | null;
  indices: Uint32Array | null;
  driver: number[] | null;
};

export function CabinetView({ positions, indices, driver }: Props) {
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const parent = host.current;
    if (!parent || !positions || !indices) {
      return;
    }
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(parent.clientWidth, 280);
    parent.replaceChildren(renderer.domElement);
    const scene = new THREE.Scene();
    scene.background = new THREE.Color("#f5f1ea");
    const camera = new THREE.PerspectiveCamera(35, parent.clientWidth / 280, 0.01, 10);
    camera.position.set(0.2, -0.45, 0.25);
    camera.lookAt(0.08, 0.09, 0.12);
    scene.add(new THREE.AmbientLight(0xffffff, 0.7));
    const light = new THREE.DirectionalLight(0xffffff, 1.2);
    light.position.set(-0.4, -0.6, 0.8);
    scene.add(light);

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geometry.setIndex(new THREE.BufferAttribute(indices, 1));
    geometry.computeVertexNormals();
    scene.add(new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({ color: "#d7c4a3", roughness: 0.7 })));
    if (driver) {
      const marker = new THREE.Mesh(
        new THREE.SphereGeometry(0.008, 16, 12),
        new THREE.MeshStandardMaterial({ color: "#2f5d50" }),
      );
      marker.position.set(driver[0], driver[1], driver[2]);
      scene.add(marker);
    }
    renderer.render(scene, camera);
    return () => {
      geometry.dispose();
      renderer.dispose();
    };
  }, [positions, indices, driver]);

  return <div ref={host} data-testid="cabinet-view" style={{ minHeight: 280 }} />;
}
