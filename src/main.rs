use std::{collections::HashSet, str::FromStr, sync::{Arc, Mutex}, time::SystemTime};
use esp_idf_hal::{delay::BLOCK, gpio::PinDriver, i2c::*};
use anyhow::Ok;
use chrono::{DateTime, Utc};
use esp_idf_hal::{delay::{Delay, FreeRtos}, i2c::I2cDriver, prelude::Peripherals};
use esp_idf_svc::{eventloop::EspSystemEventLoop, http::{client::{Request, Response}, server::EspHttpServer, Method}, nvs::{EspDefaultNvsPartition, EspNvs}, sntp::{EspSntp, SyncStatus}, wifi::{ClientConfiguration, Configuration, EspWifi}};
use esp_println::println;
use heapless::String;
use esp_idf_svc::http::server::Configuration as ServerConf;
use chrono_tz::Asia::Seoul;
use embedded_graphics::{
    mono_font::{ascii::{FONT_10X20, FONT_5X7, FONT_7X13, FONT_9X15, FONT_9X15_BOLD}, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use esp_idf_hal::i2c::config as I2cConf;
use serde_json::Value;
use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};
use esp_idf_hal::prelude::*;

static TEMP_STACK_SIZE:usize = 2000;
const WIFI_SSID:&'static str=env!("WIFI_SSID");
const WIFI_PW:&'static str=env!("WIFI_PW");
const SSD1306_ADDRESS: u8 = 0x3c;
#[derive(Debug)]
enum App_State{
    NOMAR,
    DETAILS,
    CABLESTATUS
}


fn main()-> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let delay: Delay = Default::default();
    esp_idf_svc::log::EspLogger::initialize_default();
    let nvs = EspDefaultNvsPartition::take()?;
    let i2c_conf = I2cConfig::new().baudrate(100.kHz().into());
    let mut app_state = App_State::NOMAR;
    // let i2c_conf =I2cConf::Config::new();
    let sda = peripherals.pins.gpio3;
    let scl = peripherals.pins.gpio2;
    let buttom =PinDriver::input(peripherals.pins.gpio8).unwrap();
    let mut i2c_driver = I2cDriver::new(peripherals.i2c0, sda, scl, &i2c_conf)?;
    let interface = I2CDisplayInterface::new(i2c_driver);
    let mut display: Ssd1306<I2CInterface<I2cDriver>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>> = Ssd1306::new(
        interface,
        DisplaySize128x64,
        DisplayRotation::Rotate0,
    ).into_buffered_graphics_mode();
    let mut cable1_pin = PinDriver::input_output(peripherals.pins.gpio18).unwrap();
    let mut cable2_pin = PinDriver::input_output(peripherals.pins.gpio19).unwrap();
    let mut cable3_pin = PinDriver::input_output(peripherals.pins.gpio6).unwrap();
    cable1_pin.set_low().unwrap();
    cable2_pin.set_low().unwrap();
    cable3_pin.set_low().unwrap();
    display.init().unwrap();
    display.clear_buffer();
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(BinaryColor::On)
        .build();
    let text_style2 = MonoTextStyleBuilder::new()
        .font(&FONT_7X13)
        .text_color(BinaryColor::On)
        .build();
    Text::with_baseline("Device ", Point::new(0, 20), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline("Booting", Point::new(0, 40), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();

    let time_mem = Arc::new(Mutex::new(EspNvs::new(nvs.clone(), "timedata", true).unwrap()));
    let mut wifi_driver = EspWifi::new(
        peripherals.modem,
        sys_loop,
        Some(nvs)
    )?;
    let wifi_ssid: String<32> = String::from_str(WIFI_SSID).unwrap();
    let wifi_pw: String<64> = String::from_str(&WIFI_PW).unwrap();
    wifi_driver.set_configuration(&Configuration::Client(ClientConfiguration{
        ssid: wifi_ssid,
        password: wifi_pw,
        ..Default::default()
    }))?;
    wifi_driver.start()?;
    wifi_driver.connect()?;
    while !wifi_driver.is_connected()?{
        let config = wifi_driver.get_configuration()?;
        println!("Waiting for station {:?}", config);
    };
    println!("Should be connected now");
    // display.clear_buffer();
    
    // display.flush().unwrap();
    
    let ntp = EspSntp::new_default()?;
    println!("Synchronizing with NTP Server");
    while ntp.get_sync_status() != SyncStatus::Completed {}
    println!("Time Sync Completed");
    let mem=time_mem.clone();
    let server_thread = std::thread::Builder::new()
        .stack_size(TEMP_STACK_SIZE)
        .spawn(move||webserver_thread_fuction(
            mem,
        ));
    // display.clear_buffer();
    let mem=time_mem.clone();
    // let job_thread = std::thread::Builder::new()
    //     .stack_size(TEMP_STACK_SIZE)
    //     .spawn(move||job_thread_fuction(
    //         mem,
    //     ));
    let mut flag = 0;
    let mut flag_list = vec![false,false,false,false];
    loop{
        
        // if buttom.is_low(){
        //     app_state =App_State::DETAILS;
        // }else {
        //     app_state =App_State::NOMAR;
        // }
        if buttom.is_low(){
            if flag==0{
                flag=1;
                match app_state {
                    App_State::NOMAR=>app_state =App_State::DETAILS,
                    App_State::DETAILS=>app_state =App_State::CABLESTATUS,
                    App_State::CABLESTATUS=>app_state =App_State::NOMAR,
                }
                // flag=0;
            }
        }else{
            flag=0;
        }
        display.clear_buffer();
        match app_state {
            App_State::NOMAR=>{
                Text::with_baseline("Device Info", Point::new(0, 0), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                Text::with_baseline("WIFI: ", Point::new(0, 20), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                Text::with_baseline("SERVER: ", Point::new(0, 40), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                if wifi_driver.is_connected().unwrap(){
                    Text::with_baseline("OK", Point::new(55, 20), text_style, Baseline::Top)
                        .draw(&mut display)
                        .unwrap();
                }
                if server_thread.is_ok(){
                    Text::with_baseline("OK", Point::new(70, 40), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                }
            }
            App_State::DETAILS=>{
                // display.clear_buffer();
                Text::with_baseline("SERVER IP", Point::new(0, 20), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                if wifi_driver.is_connected().unwrap(){
                    let netif =wifi_driver.sta_netif();
                    let ip_info =netif.get_ip_info().unwrap().ip.to_string();
                    Text::with_baseline(ip_info.as_str(), Point::new(0, 40), text_style, Baseline::Top)
                        .draw(&mut display)
                        .unwrap();
                }
            },
            App_State::CABLESTATUS=>{
                // display.clear_buffer();
                Text::with_baseline("", Point::zero(), text_style2, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                Text::with_baseline("C1:", Point::new(0, 13), text_style2, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                Text::with_baseline("C2:", Point::new(70, 13), text_style2, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                Text::with_baseline("C3:", Point::new(0, 39), text_style2, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                Text::with_baseline("C4:", Point::new(70, 39), text_style2, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                let mut buf = [0u8; 1024]; 
                let nvs_time=time_mem.lock().unwrap().get_str("timedata", &mut buf).unwrap().unwrap_or(r#"{"cable1":"00:00","cable2":"00:00","cable3":"00:00","cable4":"00:00"}"#);
                let parsed: Value = serde_json::from_str(nvs_time).expect("Failed to parse JSON");
                if let Value::Object(map) = parsed {
                    for (key, value) in map {
                        match key.as_str() {
                            "cable1" =>{
                                Text::with_baseline(value.as_str().unwrap(), Point::new(20, 13), text_style2, Baseline::Top)
                                    .draw(&mut display)
                                    .unwrap();
                            },
                            "cable2" =>{
                                Text::with_baseline(value.as_str().unwrap(), Point::new(90, 13), text_style2, Baseline::Top)
                                    .draw(&mut display)
                                    .unwrap();
                            }
                            "cable3"=>{
                                Text::with_baseline(value.as_str().unwrap(), Point::new(20, 39), text_style2, Baseline::Top)
                                    .draw(&mut display)
                                    .unwrap();
                            },
                            "cable4"=>{
                                Text::with_baseline(value.as_str().unwrap(), Point::new(90, 39), text_style2, Baseline::Top)
                                    .draw(&mut display)
                                    .unwrap();
                            },
                            _=>{}
                        }
                    }
                };
                
                
            }
        }
        
        // while buttom.is_low(){
            
            
        //     display.flush().unwrap();
        // }
        display.flush().unwrap();
        let mut buf = [0u8; 1024]; 
        let nvs_time=time_mem.lock().unwrap().get_str("timedata", &mut buf).unwrap().unwrap_or(r#"{"cable1":"00:00","cable2":"00:00","cable3":"00:00","cable4":"00:00"}"#);
        check_timer(nvs_time,&mut flag_list,&mut cable1_pin,&mut cable2_pin,&mut cable3_pin);
        // println!("{:?}",formatted);
        FreeRtos::delay_ms(1);
    }
    Ok(())
}


fn webserver_thread_fuction(
    time_mem:Arc<Mutex<EspNvs<esp_idf_svc::nvs::NvsDefault>>>,
){
    let conf =ServerConf::default();
    let mut server = EspHttpServer::new(&conf).unwrap();
    let mem = time_mem.clone();
    server.fn_handler("/", Method::Get, move|request| {
        let mut buf = [0u8; 1024]; 
        let time_data = mem.lock().unwrap().get_str("timedata", &mut buf).unwrap().unwrap_or(r#"{"cable1":"00:00","cable2":"00:00","cable3":"00:00","cable4":"00:00"}"#);
        let html = index_html(time_data);
        let mut response = request.into_ok_response()?;
        response.write(html.as_bytes())?;
        Ok(())
    }).unwrap();
    server.fn_handler("/data", Method::Post, move |mut request,| {
        let mut buf = [0u8; 1024]; 
        if let core::result::Result::Ok(size)=request.read(&mut buf){
            if let core::result::Result::Ok(data) = std::str::from_utf8(&buf[..size]) {
                time_mem.lock().unwrap().set_str("timedata", data).unwrap();
                println!("SERVER: {}", data);
            } else {
                println!("Invalid UTF-8 data");
            }
        }
        Ok(())
    }).unwrap();
    loop{
        
        FreeRtos::delay_ms(100);
    }
}

fn check_timer(
    nvs_data:&str,
    flag_list:&mut Vec<bool>,
    cable1:&mut PinDriver<esp_idf_hal::gpio::Gpio18, esp_idf_hal::gpio::InputOutput>,
    cable2:&mut PinDriver<esp_idf_hal::gpio::Gpio19, esp_idf_hal::gpio::InputOutput>,
    cable3:&mut PinDriver<esp_idf_hal::gpio::Gpio6, esp_idf_hal::gpio::InputOutput>,
){
    let st_now = SystemTime::now();
    let dt_now_utc: DateTime<Utc> = st_now.clone().into();
    let tz = dt_now_utc.with_timezone(&Seoul);
    let formatted = format!("{}", tz.format("%H:%M"));
    let parsed: Value = serde_json::from_str(nvs_data).expect("Failed to parse JSON");
    if let Value::Object(map) = parsed {
        for (key, value) in map {
            if formatted==value.to_string().replace('"', ""){
                match key.as_str() {
                    "cable1"=>{
                        if !flag_list[0]{
                            if cable1.is_low(){
                                cable1.set_high().unwrap();
                                FreeRtos::delay_ms(1000);
                                cable1.set_low().unwrap();
                                flag_list[0]=true;
                                continue;
                            }
                        }
                    },
                    "cable2"=>{
                        if !flag_list[1]{
                            if cable2.is_low(){
                                cable2.set_high().unwrap();
                                FreeRtos::delay_ms(1000);
                                cable2.set_low().unwrap();
                                flag_list[1]=true;
                                continue;
                            }
                        }
                    },
                    "cable3"=>{
                        if !flag_list[2]{
                            if cable3.is_low(){
                                cable3.set_high().unwrap();
                                FreeRtos::delay_ms(1000);
                                cable3.set_low().unwrap();
                                flag_list[2]=true;
                                continue;
                            }
                        }
                    },
                    "cable4"=>{
                        if !flag_list[3]{
                            
                        }
                    },
                    _=>{

                    }
                }
            }else{
                let num = key.replace("cable", "");
                let num = usize::from_str(num.as_str()).unwrap()-1;
                flag_list[num]=false;
            }
        }
    }
}

fn index_html(
    time_data:&str
) -> std::string::String {
    let parsed: Value = serde_json::from_str(time_data).expect("Failed to parse JSON");
    format!(
        r#"
    <!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Settings Page</title>
    <!-- Bootstrap CSS -->
    <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0-alpha1/dist/css/bootstrap.min.css" rel="stylesheet">
    <style>
        body {{
            background-color: #f8f9fa;
        }}
        .settings-card {{
            margin-top: 50px;
        }}
        .time-label {{
            font-weight: bold;
    }}
    </style>
</head>
<body>
    <div class="container">
        <div class="row justify-content-center">
            <div class="col-md-6">
                <div class="card settings-card shadow-sm">
                    <div class="card-header bg-primary text-white text-center">
                        <h4>Time Settings</h4>
                    </div>
                    <div class="card-body">
                        <form id="settingsForm">
                            <!-- Time Input Fields -->
                            <div class="mb-3">
                                <label for="cable1" class="form-label time-label">Cable 1</label>
                                <input type="time" class="form-control" id="cable1" name="cable1" value={} required>
                            </div>
                            <div class="mb-3">
                                <label for="cable2" class="form-label time-label">Cable 2</label>
                                <input type="time" class="form-control" id="cable2" name="cable2" value={} required>
                            </div>
                            <div class="mb-3">
                                <label for="cable3" class="form-label time-label">Cable 3</label>
                                <input type="time" class="form-control" id="cable3" name="cable3" value={} required>
                            </div>
                            <div class="mb-3">
                                <label for="cable4" class="form-label time-label">Cable 4</label>
                                <input type="time" class="form-control" id="cable4" name="cable4" value={} required>
                            </div>
                            <!-- Save Button -->
                            <div class="d-grid mt-4">
                                <button type="submit" class="btn btn-primary">Save Changes</button>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    </div>

    <!-- Bootstrap Bundle with Popper -->
    <script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0-alpha1/dist/js/bootstrap.bundle.min.js"></script>
    <script>
        document.getElementById('settingsForm').addEventListener('submit', async function (event) {{
            event.preventDefault();

            // Collect form data
            const formData = new FormData(this);
            const data = Object.fromEntries(formData.entries());

            try {{
                // Send POST request
                const response = await fetch('/data', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify(data)
    }});

                if (response.ok) {{
                    alert('Settings saved successfully!');
    }} else {{
                    alert('Failed to save settings. Please try again.');
    }}
    }} catch (error) {{
                console.error('Error:', error);
                alert('An error occurred. Please check your network connection.');
    }}
    }});
    </script>
</body>
</html>
    "#
        ,parsed.get("cable1").unwrap(),parsed.get("cable2").unwrap(),parsed.get("cable3").unwrap(),parsed.get("cable4").unwrap())
}

