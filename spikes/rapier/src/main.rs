//! Spike: does Rapier 2D (f64, enhanced-determinism) give the textbook answers sci_anim's
//! lessons need? Prints one line per lesson with the measured and the expected numbers.

use rapier2d_f64::prelude::*;

fn world(gravity: f64) -> PhysicsWorld {
    let mut world = PhysicsWorld::default();
    world.gravity = Vector::new(0.0, -gravity);
    world.integration_parameters.dt = 1.0 / 480.0;
    if std::env::var("TUNED").is_ok() {
        world.integration_parameters.num_solver_iterations = 16;
        world.integration_parameters.warmstart_coefficient = 0.0;
    }
    world
}

fn cart(world: &mut PhysicsWorld, x: f64, v: f64, mass: f64, e: f64) -> RigidBodyHandle {
    world
        .insert(
            RigidBodyBuilder::dynamic()
                .translation(Vector::new(x, 0.0))
                .linvel(Vector::new(v, 0.0)),
            ColliderBuilder::cuboid(0.5, 0.3)
                .mass(mass)
                .restitution(e)
                .restitution_combine_rule(CoefficientCombineRule::Min)
                .friction(0.0),
        )
        .0
}

fn carts(e: f64) {
    let mut world = world(0.0);
    let a = cart(&mut world, -2.0, 3.0, 2.0, e);
    let b = cart(&mut world, 0.0, 0.0, 1.0, e);
    for _ in 0..480 {
        world.step();
    }
    let (va, vb) = (world.bodies[a].linvel().x, world.bodies[b].linvel().x);
    let (wa, wb) = (world.bodies[a].angvel(), world.bodies[b].angvel());
    let p = 2.0 * va + vb;
    let ke = 0.5 * 2.0 * va * va + 0.5 * vb * vb;
    let (ea, eb, eke) = if e == 1.0 {
        (1.0, 4.0, 9.0)
    } else {
        (2.0, 2.0, 6.0)
    };
    println!(
        "carts e={e}: va={va:.6} (want {ea}) vb={vb:.6} (want {eb}) p={p:.6} (want 6) ke={ke:.6} (want {eke}) spins={wa:.2e},{wb:.2e}"
    );
}

fn cradle() {
    let mut world = world(9.81);
    let r = 0.25;
    let length = 2.0;
    let mut balls = Vec::new();
    for k in 0..5 {
        let pivot_x = k as f64 * 2.0 * r;
        let anchor = world.insert(
            RigidBodyBuilder::fixed().translation(Vector::new(pivot_x, length)),
            ColliderBuilder::ball(0.01).sensor(true),
        );
        let angle: f64 = if k == 0 { -0.5 } else { 0.0 };
        let ball = world.insert(
            RigidBodyBuilder::dynamic()
                .translation(Vector::new(pivot_x + length * angle.sin(), length - length * angle.cos())),
            ColliderBuilder::ball(r)
                .mass(1.0)
                .restitution(1.0)
                .restitution_combine_rule(CoefficientCombineRule::Max)
                .friction(0.0),
        );
        let joint = RevoluteJointBuilder::new()
            .local_anchor1(Vector::new(0.0, 0.0))
            .local_anchor2(Vector::new(
                -length * angle.sin(),
                length * angle.cos(),
            ));
        world.impulse_joints.insert(anchor.0, ball.0, joint, true);
        balls.push(ball.0);
    }
    // The first ball reaches the others after a quarter swing, about 0.71 s; read just after.
    let mut most = [0.0f64; 5];
    for _ in 0..(480.0 * 1.0) as usize {
        world.step();
        for (k, ball) in balls.iter().enumerate() {
            most[k] = most[k].max(world.bodies[*ball].linvel().length());
        }
    }
    let speeds: Vec<String> = balls
        .iter()
        .map(|b| format!("{:.3}", world.bodies[*b].linvel().length()))
        .collect();
    let ideal = (2.0 * 9.81 * length * (1.0 - 0.5f64.cos())).sqrt();
    println!(
        "cradle after 1 s: speeds {:?}; most speed each ball reached {:?}; ideal: only ball 4 moves, at {ideal:.3}",
        speeds,
        most.map(|s| (s * 1000.0).round() / 1000.0)
    );
}

fn ramp() {
    let mut world = world(9.81);
    let angle = 30f64.to_radians();
    world.insert(
        RigidBodyBuilder::fixed().rotation(-angle),
        ColliderBuilder::cuboid(20.0, 0.1).friction(0.3),
    );
    let (s, c) = angle.sin_cos();
    let block = world
        .insert(
            RigidBodyBuilder::dynamic()
                .translation(Vector::new(-c * 0.0 - s * 0.35, s * 0.0 + c * 0.35))
                .rotation(-angle),
            ColliderBuilder::cuboid(0.25, 0.25).mass(1.0).friction(0.3),
        )
        .0;
    for _ in 0..480 {
        world.step();
    }
    let v = world.bodies[block].linvel().length();
    let want = 9.81 * (s - 0.3 * c);
    println!("ramp: speed after 1 s {v:.4}, want {want:.4} (g(sin - mu cos) t)");
}

fn determinism() {
    let run = || {
        let mut world = world(9.81);
        world.insert(
            RigidBodyBuilder::fixed(),
            ColliderBuilder::cuboid(20.0, 0.1),
        );
        let mut handles = Vec::new();
        for k in 0..20 {
            handles.push(
                world
                    .insert(
                        RigidBodyBuilder::dynamic()
                            .translation(Vector::new((k % 5) as f64 * 0.61, 1.0 + (k / 5) as f64 * 0.7))
                            .rotation(k as f64 * 0.3),
                        ColliderBuilder::cuboid(0.3, 0.2).restitution(0.4).friction(0.5),
                    )
                    .0,
            );
        }
        for _ in 0..2000 {
            world.step();
        }
        handles
            .iter()
            .map(|h| world.bodies[*h].translation().x.to_bits())
            .collect::<Vec<u64>>()
    };
    println!("determinism in one process: {}", run() == run());
}

fn main() {
    carts(1.0);
    carts(0.0);
    cradle();
    ramp();
    determinism();
}
