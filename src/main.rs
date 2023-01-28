use tokio;
use tokio_serial;
use rumqttc::{MqttOptions, AsyncClient, QoS};
use solis::Shutdown;
use std::time::SystemTime;

fn vec_u16_to_u32(i: &[u16]) -> u32 {
   (((i[0] as u32) << 16) + i[1] as u32).into()
}

#[derive(Debug)]
struct Register<'a> {
    name: &'a str,
    unit: &'a str,
    offset: u16,
    num_bits: usize,
    num_decimals: usize,
    value: f32,
}

/* 
AlltimeEnergy_KW_z = solis1500.read_long(3008, functioncode=4, signed=False) # Read All Time Energy (KWH Total) as Unsigned 32-Bit
print("Generated (All time):" +str(AlltimeEnergy_KW_z) + "kWh")
Today_KW_z = solis1500.read_register(3014, functioncode=4, signed=False) # Read Today Energy (KWH Total) as 16-Bit
print("Generated (Today) :" + str(Today_KW_z/10) + "kWh")
*/

pub async fn run_mqtt_eventloop(mut eventloop: rumqttc::EventLoop, shutdown: Shutdown) {
    while !shutdown.is_shutdown() {
        let notification = eventloop.poll().await.unwrap();
        println!("MQTT = {:?}", notification);
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use tokio_serial::SerialStream;

    use tokio_modbus::prelude::*;

    let (notify_shutdown, _) = tokio::sync::broadcast::channel(1);
    let tty_path = "/dev/ttyUSB0";
    let slave = Slave(0x01);
    let mut mqttoptions = MqttOptions::new("solis-logger", "mqtt", 1883);
    mqttoptions.set_keep_alive(std::time::Duration::from_secs(5));

    let registers = vec![
        Register {
            name: "Total power generation",
            unit: "kWh",
            offset: 3008,
            num_bits: 32,
            num_decimals: 0,
            value: 0.0,
        },
        Register {
            name: "kWh today",
            unit: "kWh",
            offset: 3014,
            num_bits: 16,
            num_decimals: 1,
            value: 0.0,
        },
        Register {
            name: "kWh yesterday",
            unit: "kWh",
            offset: 3015,
            num_bits: 16,
            num_decimals: 1,
            value: 0.0,
        },
        Register {
            name: "kWh this month",
            unit: "kWh",
            offset: 3010,
            num_bits: 32,
            num_decimals: 0,
            value: 0.0,
        },
        Register {
            name: "kWh last month",
            unit: "kWh",
            offset: 3012,
            num_bits: 32,
            num_decimals: 0,
            value: 0.0,
        },

        Register {
            name: "AC power",
            unit: "W",
            offset: 3004,
            num_bits: 32,
            num_decimals: 0,
            value: 0.0,
        },

        Register {
            name: "DC1 voltage",
            unit: "V",
            offset: 3021,
            num_bits: 16,
            num_decimals: 1,
            value: 0.0,
        },
        Register {
            name: "DC1 current",
            unit: "A",
            offset: 3022,
            num_bits: 16,
            num_decimals: 1,
            value: 0.0,
        },
        Register {
            name: "DC2 voltage",
            unit: "V",
            offset: 3023,
            num_bits: 16,
            num_decimals: 1,
            value: 0.0,
        },
        Register {
            name: "DC2 current",
            unit: "A",
            offset: 3024,
            num_bits: 16,
            num_decimals: 1,
            value: 0.0,
        },

        Register {
            name: "AC voltage",
            unit: "V",
            offset: 3035,
            num_bits: 16,
            num_decimals: 1,
            value: 0.0,
        },

        Register {
            name: "Temperature",
            unit: "°C",
            offset: 3041,
            num_bits: 16,
            num_decimals: 1,
            value: 0.0,
        },

        Register {
            name: "AC frequency",
            unit: "Hz",
            offset: 3042,
            num_bits: 16,
            num_decimals: 2,
            value: 0.0,
        },
];

    let builder = tokio_serial::new(tty_path, 9600)
        .data_bits(tokio_serial::DataBits::Eight)
        .parity(tokio_serial::Parity::None)
        .stop_bits(tokio_serial::StopBits::One)
        .timeout(std::time::Duration::new(5, 0));
    let port = SerialStream::open(&builder)?;
    
    let mut ctx = rtu::connect_slave(port, slave).await?;
    let (client, eventloop) = AsyncClient::new(mqttoptions, 10);

    let _mqtt_eventloop_thread = tokio::spawn(run_mqtt_eventloop(eventloop, Shutdown::new(notify_shutdown.subscribe())));

    println!("Collecting data");
    
    for mut r in registers {

        let rsp = tokio::time::timeout(std::time::Duration::from_secs(5), ctx.read_input_registers(r.offset, (r.num_bits/16) as u16)).await??;
        let x = match r.num_bits {
            16 => rsp[0] as u32,
            32 => vec_u16_to_u32(&rsp),
            _ => 0 // xxx: should be error
        };
        r.value = match r.num_decimals {
            0 => x as f32,
            1 => x as f32 / 10.0,
            2 => x as f32 / 100.0,
            _ => x as f32,
        };
        
        println!("{}: {} {}", r.name, r.value, r.unit);
        let mqtt_topic = format!("meters/solis/{}_{}", r.name.to_lowercase().replace(" ", "_"),
                                 r.unit.to_lowercase().replace("°", ""));
        client
            .publish(mqtt_topic, QoS::AtLeastOnce, false, format!("{}", r.value))
            .await?
    }

    let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
    client
        .publish("meters/solis/online", QoS::AtLeastOnce, false, format!("{}", t.as_secs()))
            .await?;

    /* Debug */

    for n in 3000..3099 {
//    for n in 3000..3030 {

/*        if ignore.contains(n) {
            continue;
        }
  */      
        let rsp = tokio::time::timeout(std::time::Duration::from_secs(5), ctx.read_input_registers(n, 1)).await??;
        println!("register {} (u16): {:?}", n, rsp);
        let rsp = tokio::time::timeout(std::time::Duration::from_secs(5), ctx.read_input_registers(n, 2)).await??;
        let x = vec_u16_to_u32(&rsp);
        println!("register {} (u32): {:?} {}", n, rsp, x);
    }

    drop(notify_shutdown);
    Ok(())
}

/*

register 3000 (u16): [30] - slavefirmwareversion
register 3001 (u16): [253] - mainfirmwareversion
register 3002 (u16): [1]
register 3002 (u32): [1, 1] 65537
register 3003 (u16): [1]
register 3003 (u32): [1, 0] 65536
register 3004 (u32): [0, 240] 240 - AC Power

register 3006 (u16): [0]
register 3006 (u32): [0, 384] 384 - on off, mode 3? https://github.com/hvoerman/solis2mqtt/blob/master/solis_modbus.yaml#L249 
register 3007 (u16): [384] - pv power (?) - https://github.com/m0wmt/solar/blob/main/solis_meter.py#L73

register 3008 (u32): [0, 26368] 26368 - kwh total
register 3010 (u32): [0, 719] 719 - kwh this month
register 3012 (u32): [0, 1423] 1423 - kwh last month
register 3014 (u16): [373] - kwh today / 10
register 3015 (u16): [146] - kwh yesterday / 16

register 3016 (u32): [0, 7334] 7334 - production this year?
register 3018 (u32): [0, 8842] 8842 - production last year? (kwh)

register 3020 (u16): [0]
register 3020 (u32): [0, 4400] 4400
register 3021 (u16): [4400]
register 3021 (u32): [4400, 6] 288358406
register 3022 (u16): [6]
register 3022 (u32): [6, 3834] 397050
register 3023 (u16): [3834]
register 3023 (u32): [3834, 3] 251265027
register 3024 (u16): [3]
register 3024 (u32): [3, 0] 196608
register 3025 (u16): [0]
register 3025 (u32): [0, 0] 0
register 3026 (u16): [0]
register 3026 (u32): [0, 0] 0
register 3027 (u16): [0]
register 3027 (u32): [0, 0] 0
register 3028 (u16): [0]
register 3028 (u32): [0, 0] 0
register 3029 (u16): [0]
register 3029 (u32): [0, 0] 0
register 3030 (u16): [0]
register 3030 (u32): [0, 4745] 4745
register 3031 (u16): [4745]

register 3032 (u16): [2366] - V_a / 10 ?
register 3033 (u16): [2437] - V_b / 10 ?
register 3034 (u16): [2435] - V_c / 10 ?
register 3035 (u16): [2456] - AC volt / 10 ?
register 3036 (u16): [5] - AC current_A / 10 ?
register 3037 (u16): [5] - AC current_C / 10 ?
register 3038 (u16): [6] - AC current_C / 10 ?
register 3039 (u16): [0]
register 3039 (u32): [0, 0] 0
register 3040 (u16): [0]

register 3041 (u16): [360] - temperature C / 10 https://github.com/m0wmt/solar/blob/main/solis_meter.py#L62

register 3042 (u16): [5000] - AC frequency (hz) / 100?

register 3043 (u16): [3]
register 3043 (u32): [3, 0] 196608
register 3044 (u16): [0]
register 3044 (u32): [0, 11000] 11000 - maxoutputpower
register 3046 (u16): [0]
register 3046 (u32): [0, 0] 0
register 3047 (u16): [0]
register 3047 (u32): [0, 4] 4
register 3048 (u16): [4]
register 3048 (u32): [4, 11000] 273144
register 3049 (u16): [11000] - max output power?
register 3050 (u16): [1000] - max input voltage?
register 3051 (u16): [1000] - power limitation? https://github.com/hvoerman/solis2mqtt/blob/master/solis_modbus.yaml#L232 
register 3051 (u32): [1000, 0] 65536000
register 3052 (u16): [0]
register 3052 (u32): [0, 16] 16
register 3052 (u32): [0, 16] 16
register 3053 (u16): [16]
register 3053 (u32): [16, 4] 1048580
register 3054 (u16): [4]
register 3054 (u32): [4, 0] 262144
register 3055 (u16): [0]
register 3055 (u32): [0, 0] 0
register 3056 (u16): [0]
register 3056 (u32): [0, 0] 0
register 3057 (u16): [0]
register 3057 (u32): [0, 240] 240
register 3058 (u16): [240]
register 3058 (u32): [240, 1000] 15729640
register 3059 (u16): [1000]
register 3059 (u32): [1000, 32785] 65568785
register 3060 (u16): [32785]
register 3060 (u32): [32785, 37163] 2148634923
register 3061 (u16): [37163]
register 3061 (u32): [37163, 37] 2435514405
register 3062 (u16): [37]
register 3062 (u32): [37, 528] 2425360
register 3063 (u16): [528]
register 3063 (u32): [528, 0] 34603008
register 3064 (u16): [0]
register 3064 (u32): [0, 2] 2
register 3065 (u16): [2]
register 3065 (u32): [2, 0] 131072
register 3066 (u16): [0]
register 3066 (u32): [0, 0] 0
register 3067 (u16): [0]
register 3067 (u32): [0, 0] 0
register 3068 (u16): [0]
register 3068 (u32): [0, 0] 0
register 3069 (u16): [0]
register 3069 (u32): [0, 0] 0
register 3070 (u16): [0]
register 3070 (u32): [0, 1] 1
register 3071 (u16): [1]
register 3072 (u16): [22] - year
register 3073 (u16): [8] - month
register 3074 (u16): [17] - day
register 3075 (u16): [19] - time - hour - offset CEST?
register 3076 (u16): [25] - time - minutes
register 3077 (u16): [45] - time - seconds
register 3078 (u16): [0]
register 3078 (u32): [0, 0] 0
register 3079 (u16): [0]
register 3079 (u32): [0, 0] 0
register 3080 (u16): [0]
register 3080 (u32): [0, 0] 0
register 3081 (u16): [0]
register 3081 (u32): [0, 0] 0
register 3082 (u16): [0]
register 3082 (u32): [0, 0] 0
register 3083 (u16): [0]
register 3083 (u32): [0, 0] 0
register 3084 (u16): [0]
register 3084 (u32): [0, 0] 0
register 3085 (u16): [0]
register 3085 (u32): [0, 0] 0
register 3086 (u16): [0]
register 3086 (u32): [0, 2] 2
register 3087 (u16): [2]
register 3087 (u32): [2, 1] 131073
register 3088 (u16): [1]
register 3088 (u32): [1, 170] 65706
register 3089 (u16): [170]
register 3089 (u32): [170, 85] 11141205
register 3090 (u16): [85]
register 3090 (u32): [85, 0] 5570560
register 3091 (u16): [0]
register 3091 (u32): [0, 0] 0
register 3092 (u16): [0]
register 3092 (u32): [0, 0] 0
register 3093 (u16): [0]
register 3093 (u32): [0, 0] 0
register 3094 (u16): [0]
register 3094 (u32): [0, 0] 0
register 3095 (u16): [0]
register 3095 (u32): [0, 0] 0
register 3096 (u16): [0]
register 3096 (u32): [0, 0] 0
register 3097 (u16): [0]
register 3097 (u32): [0, 0] 0
register 3098 (u16): [0]
register 3098 (u32): [0, 0] 0
3229: serial number https://github.com/hvoerman/solis2mqtt/blob/master/solis_modbus.yaml#L217

*/
