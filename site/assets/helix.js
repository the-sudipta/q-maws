// Hero background: a slowly turning DNA double helix (three.js).
import * as THREE from 'https://cdn.jsdelivr.net/npm/three@0.160.0/build/three.module.js';

const canvas = document.getElementById('helix');
const reduce = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
if (canvas) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(40, 1, 0.1, 100);
  camera.position.set(0, 0, 18);

  const group = new THREE.Group();
  scene.add(group);
  const navy = new THREE.Color('#1f4e79');
  const orange = new THREE.Color('#d9731a');
  const teal = new THREE.Color('#23877a');
  const sphere = new THREE.SphereGeometry(0.16, 16, 16);
  const pairs = 46, turns = 3.2, height = 15, radius = 2.1;
  for (let i = 0; i < pairs; i++) {
    const t = i / (pairs - 1);
    const a = t * turns * Math.PI * 2;
    const y = (t - 0.5) * height;
    const p1 = new THREE.Vector3(Math.cos(a) * radius, y, Math.sin(a) * radius);
    const p2 = new THREE.Vector3(Math.cos(a + Math.PI) * radius, y, Math.sin(a + Math.PI) * radius);
    // every fourth rung in orange: the "missing words"
    const missing = i % 4 === 2;
    const m1 = new THREE.MeshBasicMaterial({ color: navy, transparent: true, opacity: 0.85 });
    const m2 = new THREE.MeshBasicMaterial({ color: missing ? orange : teal, transparent: true, opacity: 0.85 });
    const s1 = new THREE.Mesh(sphere, m1); s1.position.copy(p1); group.add(s1);
    const s2 = new THREE.Mesh(sphere, m2); s2.position.copy(p2); group.add(s2);
    const rung = new THREE.BufferGeometry().setFromPoints([p1, p2]);
    // the orange rungs are left open: the "absent words"
    if (!missing) group.add(new THREE.Line(rung, new THREE.LineBasicMaterial({ color: navy, transparent: true, opacity: 0.18 })));
  }
  // soft background points
  const pts = new THREE.BufferGeometry();
  const n = 260, pos = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    pos[i * 3] = (Math.random() - 0.5) * 40;
    pos[i * 3 + 1] = (Math.random() - 0.5) * 24;
    pos[i * 3 + 2] = (Math.random() - 0.5) * 20 - 6;
  }
  pts.setAttribute('position', new THREE.BufferAttribute(pos, 3));
  scene.add(new THREE.Points(pts, new THREE.PointsMaterial({ color: navy, size: 0.06, transparent: true, opacity: 0.35 })));

  group.rotation.z = -0.5;
  function layout() {
    const w = canvas.clientWidth, h = canvas.clientHeight;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    group.position.x = w > 900 ? 8.2 : 0;
    group.scale.setScalar(w > 900 ? 1 : 0.75);
  }
  layout();
  window.addEventListener('resize', layout);
  let mx = 0, my = 0;
  window.addEventListener('pointermove', e => {
    mx = (e.clientX / window.innerWidth - 0.5);
    my = (e.clientY / window.innerHeight - 0.5);
  });
  function frame() {
    group.rotation.y += reduce ? 0 : 0.0035;
    group.rotation.x += ((my * 0.3) - group.rotation.x) * 0.03;
    camera.position.x += ((mx * 1.5) - camera.position.x) * 0.03;
    camera.lookAt(0, 0, 0);
    renderer.render(scene, camera);
    requestAnimationFrame(frame);
  }
  frame();
}
