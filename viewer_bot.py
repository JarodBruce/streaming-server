import argparse
import threading
import requests
import time
import uuid
import logging

# --- Configuration ---
BASE_URL = "http://localhost:8080"
VIDEO_URL = f"{BASE_URL}/video/sample.mp4"
HEARTBEAT_URL = f"{BASE_URL}/heartbeat"
HEARTBEAT_INTERVAL = 5  # seconds

# --- Logging Setup ---
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(threadName)s - %(message)s'
)

def viewer_worker(worker_id: int):
    """
    Simulates a single viewer.
    - Sends periodic heartbeats.
    - Continuously streams a video.
    """
    client_id = str(uuid.uuid4())
    thread_name = f"Worker-{worker_id}"
    threading.current_thread().name = thread_name
    
    logging.info(f"Started with Client ID: {client_id}")

    with requests.Session() as session:
        last_heartbeat_time = 0
        
        while True:
            try:
                # --- Send Heartbeat ---
                if time.time() - last_heartbeat_time > HEARTBEAT_INTERVAL:
                    try:
                        session.post(HEARTBEAT_URL, json={"client_id": client_id}, timeout=2)
                        # logging.info("Sent heartbeat.")
                        last_heartbeat_time = time.time()
                    except requests.RequestException as e:
                        logging.error(f"Heartbeat failed: {e}")

                # --- Stream Video ---
                try:
                    with session.get(VIDEO_URL, stream=True, timeout=10) as r:
                        r.raise_for_status()
                        # logging.info("Started streaming video.")
                        for chunk in r.iter_content(chunk_size=8192):
                            # Process chunk (e.g., pass to a media player)
                            # In this simulation, we just consume it.
                            pass
                        # logging.info("Finished streaming video.")
                except requests.RequestException as e:
                    logging.error(f"Streaming failed: {e}")

                # Simulate a short pause before re-watching
                time.sleep(1)

            except Exception as e:
                logging.error(f"An unexpected error occurred: {e}")
                time.sleep(5) # Wait before retrying on major errors


def main():
    """
    Main function to parse arguments and start viewer threads.
    """
    parser = argparse.ArgumentParser(description="Viewer Bot for Streaming Server Load Testing")
    parser.add_argument(
        "-w", "--workers",
        type=int,
        default=1,
        help="Number of concurrent viewers (workers) to simulate."
    )
    args = parser.parse_args()

    logging.info(f"Starting {args.workers} viewer worker(s)...")

    threads = []
    for i in range(args.workers):
        thread = threading.Thread(target=viewer_worker, args=(i + 1,), daemon=True)
        threads.append(thread)
        thread.start()
        time.sleep(0.05) # Stagger thread starts slightly

    # Keep the main thread alive to let workers run
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        logging.info("Stopping viewer bot...")

if __name__ == "__main__":
    main()
