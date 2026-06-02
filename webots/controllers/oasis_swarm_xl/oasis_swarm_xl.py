"""
OASIS Swarm XL — 5 drones, wind, 8 obstacles, large arena
Federation: drones share fear zones + wind data in real-time.
Wind: simulated random gusts that push the drones laterally.
The wind synapse learns to compensate — shared via federation.
"""

import sys, os, json, math, time, random
from controller import Robot

# PID cerebellum
class PID:
    def __init__(self):
        self.pvxe=0;self.pvye=0;self.pae=0;self.ppe=0;self.pre=0;self.ai=0
    def pid(self,dt,dvx,dvy,dyaw,dalt,roll,pitch,yr,alt,vx,vy):
        g={"kp_att_y":1,"kd_att_y":0.5,"kp_att_rp":0.5,"kd_att_rp":0.1,
           "kp_vel_xy":2,"kd_vel_xy":0.5,"kp_z":10,"ki_z":5,"kd_z":5}
        vxe=dvx-vx;vxd=(vxe-self.pvxe)/dt;vye=dvy-vy;vyd=(vye-self.pvye)/dt
        dp=g["kp_vel_xy"]*max(-1,min(1,vxe))+g["kd_vel_xy"]*vxd
        dr=-g["kp_vel_xy"]*max(-1,min(1,vye))-g["kd_vel_xy"]*vyd
        self.pvxe=vxe;self.pvye=vye
        ae=dalt-alt;ad=(ae-self.pae)/dt;self.ai+=ae*dt
        ac=g["kp_z"]*ae+g["kd_z"]*ad+g["ki_z"]*max(-2,min(2,self.ai))+48;self.pae=ae
        pe=dp-pitch;pd=(pe-self.ppe)/dt;re=dr-roll;rd=(re-self.pre)/dt;ye=dyaw-yr
        rc=g["kp_att_rp"]*max(-1,min(1,re))+g["kd_att_rp"]*rd
        pc=-g["kp_att_rp"]*max(-1,min(1,pe))-g["kd_att_rp"]*pd
        yc=g["kp_att_y"]*max(-1,min(1,ye))
        self.ppe=pe;self.pre=re
        return[max(0,min(600,ac-rc+pc+yc)),max(0,min(600,ac-rc-pc-yc)),
               max(0,min(600,ac+rc-pc+yc)),max(0,min(600,ac+rc+pc-yc))]

# Federation
SHARED="C:/dev/oasis/webots/swarm_xl_shared/"
os.makedirs(SHARED,exist_ok=True)

def broadcast(name, fears, wind_comp):
    with open(os.path.join(SHARED,f"{name}.json"),"w") as f:
        json.dump({"src":name,"t":time.time(),"fears":fears,"wind":wind_comp},f)

def receive(my_name):
    fears=[]; winds=[]
    for fn in os.listdir(SHARED):
        if fn.endswith(".json") and not fn.startswith(my_name):
            try:
                with open(os.path.join(SHARED,fn)) as f:
                    d=json.load(f)
                    fears.extend(d.get("fears",[])); winds.append(d.get("wind",[0,0]))
            except: pass
    return fears, winds

# Wind simulation
class Wind:
    def __init__(self, seed):
        self.rng = random.Random(seed)
        self.wx = 0.0; self.wy = 0.0
        self.gust_timer = 0
    def update(self):
        self.gust_timer -= 1
        if self.gust_timer <= 0:
            # New gust every 100-500 ticks
            self.wx = self.rng.uniform(-0.04, 0.04)  # gentler gusts
            self.wy = self.rng.uniform(-0.04, 0.04)
            self.gust_timer = self.rng.randint(200, 600)
        return self.wx, self.wy

if __name__ == '__main__':
    robot = Robot()
    ts = int(robot.getBasicTimeStep())
    args = sys.argv[1:] if len(sys.argv)>1 else ["alpha","0"]
    name = args[0]; did = int(args[1]) if len(args)>1 else 0

    log_file = open(f"C:/dev/oasis/webots/swarm_xl_{name}.log","w",encoding="utf-8")
    def log(m): print(f"[{name}] {m}"); log_file.write(m+"\n"); log_file.flush()

    motors=[]
    for mn in ["m1_motor","m2_motor","m3_motor","m4_motor"]:
        m=robot.getDevice(mn);m.setPosition(float('inf'))
        m.setVelocity(-1 if mn in["m1_motor","m3_motor"] else 1);motors.append(m)

    imu=robot.getDevice("inertial_unit");imu.enable(ts)
    gps=robot.getDevice("gps");gps.enable(ts)
    gyro=robot.getDevice("gyro");gyro.enable(ts)
    rf_s=robot.getDevice("range_front");rf_s.enable(ts)
    rl_s=robot.getDevice("range_left");rl_s.enable(ts)
    rr_s=robot.getDevice("range_right");rr_s.enable(ts)
    rb_s=robot.getDevice("range_back");rb_s.enable(ts)

    pid=PID(); wind=Wind(did*42+7)

    # Each drone: incremental exploration (max 40cm per step, bio: cautious exploration)
    start_pos = [(0,0),(-1.0,0.5),(0.5,-1.0),(1.0,0.8),(-0.5,-0.8)][did]
    rng = random.Random(did*17+3)
    WP = [(0, 1.0, start_pos[0], start_pos[1], "TAKEOFF"),
          (250, 1.0, start_pos[0], start_pos[1], "STABLE")]
    cx, cy = start_pos[0], start_pos[1]
    t_wp = 400
    for i in range(16):
        # Step max 40cm from current position (stay in arena ±1.2m)
        dx = rng.uniform(-0.4, 0.4); dy = rng.uniform(-0.4, 0.4)
        cx = max(-1.2, min(1.2, cx + dx)); cy = max(-1.2, min(1.2, cy + dy))
        alt = rng.choice([0.7, 0.8, 1.0, 1.0, 1.2])
        WP.append((t_wp, alt, round(cx,2), round(cy,2), f"EXP{i}"))
        t_wp += 250
    WP.append((t_wp, 1.0, start_pos[0], start_pos[1], "HOME"))
    WP.append((t_wp+400, 1.0, start_pos[0], start_pos[1], "HOVER"))

    s_alt=1.0; s_x=start_pos[0]; s_y=start_pos[1]
    wp_i=0; tick=0; px=start_pos[0]; py=start_pos[1]
    fear=0.0; fear_zones=[]; foreign_fears=[]; foreign_winds=[]
    wind_comp=[0.0, 0.0]  # learned wind compensation
    total_wind_x=0.0; total_wind_y=0.0

    log(f"=== OASIS SWARM XL: {name} (id={did}) ===")
    log(f"  5 drones, 8 pillars, 4 walls, wind gusts")
    log(f"  Start: ({start_pos[0]}, {start_pos[1]})")

    while robot.step(ts)!=-1:
        tick+=1; dt=ts/1000.0
        roll,pitch,yaw=imu.getRollPitchYaw()
        gx,gy,gz=gyro.getValues()
        x,y,alt=gps.getValues()
        vx=(x-px)/dt if tick>1 else 0.0; vy=(y-py)/dt if tick>1 else 0.0
        px=x; py=y
        rf=rf_s.getValue()/1000; rl=rl_s.getValue()/1000
        rr=rr_s.getValue()/1000; rb=rb_s.getValue()/1000
        obs=min(rf,rl,rr,rb)

        # Wind gust
        wx, wy = wind.update()
        total_wind_x += wx; total_wind_y += wy

        # Waypoints
        t_alt=WP[wp_i][1]
        while wp_i<len(WP)-1 and tick>=WP[wp_i+1][0]:
            wp_i+=1; _,t_alt,tx,ty,wn=WP[wp_i]
            log(f"  >>> {wn} alt={t_alt} pos=({tx:.1f},{ty:.1f})")
        _,_,t_x,t_y,wn=WP[wp_i]

        s_alt+=(t_alt-s_alt)*0.2; s_x+=(t_x-s_x)*0.3; s_y+=(t_y-s_y)*0.3
        ae=alt-s_alt; stable=abs(ae)<0.2

        # Local fear
        lf=0.0
        if obs<0.25: lf=(0.25-obs)*4.0
        if lf>0.3: fear_zones=[(x,y,lf)]+fear_zones[:19]

        # Foreign fear
        ff=0.0
        if tick%30==0: foreign_fears,foreign_winds=receive(name)
        for fz in foreign_fears:
            d=math.sqrt((x-fz[0])**2+(y-fz[1])**2)
            if d<0.6: ff=max(ff, fz[2]*(0.6-d)*2.0)

        # Foreign wind compensation
        if foreign_winds:
            for fw in foreign_winds:
                wind_comp[0]=wind_comp[0]*0.95+fw[0]*0.05
                wind_comp[1]=wind_comp[1]*0.95+fw[1]*0.05

        fear=min(5.0, fear*0.85+max(lf,ff*0.7)*0.15)

        # Navigation + wind compensation
        dvx=0.0; dvy=0.0
        if stable:
            dx=s_x-x; dy=s_y-y; dist=math.sqrt(dx*dx+dy*dy)
            if dist>0.02:
                spd=min(0.3, dist*0.6)
                if fear>1.0: spd*=max(0.3, 1.0-fear*0.05)
                dvx=(dx/dist)*spd; dvy=(dy/dist)*spd
        # Wind compensation (own + learned from swarm)
        dvx+=wx+wind_comp[0]*0.5; dvy+=wy+wind_comp[1]*0.5
        # Learn wind compensation: oppose drift
        wind_comp[0]=wind_comp[0]*0.99+(-vx*0.01 if abs(vx)>0.05 else 0)
        wind_comp[1]=wind_comp[1]*0.99+(-vy*0.01 if abs(vy)>0.05 else 0)

        # Obstacle avoidance
        if rf<0.25: dvx-=(0.25-rf)*0.5
        if rb<0.25: dvx+=(0.25-rb)*0.5
        if rl<0.25: dvy-=(0.25-rl)*0.5
        if rr<0.25: dvy+=(0.25-rr)*0.5
        dvx=max(-0.3,min(0.3,dvx)); dvy=max(-0.3,min(0.3,dvy))
        if obs<0.12: dvx=0;dvy=0

        # Graceful degradation
        spd_t=math.sqrt(vx*vx+vy*vy)
        if spd_t>1.5 or abs(ae)>2.0:
            dvx=-vx*0.5; dvy=-vy*0.5; s_alt=max(0.3,min(2.0,alt))
            s_x=x; s_y=y; pid.ai*=0.5

        mp=pid.pid(dt,dvx,dvy,0,s_alt,roll,pitch,gz,alt,vx,vy)
        motors[0].setVelocity(-mp[0]);motors[1].setVelocity(mp[1])
        motors[2].setVelocity(-mp[2]);motors[3].setVelocity(mp[3])

        # Broadcast every 50 ticks
        if tick%50==0 and (len(fear_zones)>0 or abs(wind_comp[0])>0.001):
            broadcast(name, fear_zones, wind_comp)

        if tick%10==0:
            ffstr=f"FF:{ff:.1f}" if ff>0.01 else ""
            wstr=f"W:{wx:.2f},{wy:.2f}" if abs(wx)>0.01 or abs(wy)>0.01 else ""
            log(f"[T{tick:5}] alt:{alt:.2f} x:{x:.2f} y:{y:.2f} f:{fear*100:.0f}% "
                f"obs:{obs:.2f} {ffstr} {wstr} | {wn}")
