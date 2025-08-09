import requests
import time
import uuid
import random
import sys

BASE_URL = "http://localhost:8080"

def main():
    client_id = f"bot-{uuid.uuid4()}"
    print(f"Starting bot with client_id: {client_id}")

    while True:
        try:
            # Send heartbeat
            requests.post(f"{BASE_URL}/heartbeat", json={"client_id": client_id}, timeout=3)
            print(f"[{client_id}] Sent heartbeat.")

            # Occasionally, "watch" a video to make it more realistic
            if random.random() < 0.1: # 10% chance
                try:
                    videos_res = requests.get(f"{BASE_URL}/videos", timeout=3)
                    videos = videos_res.json()
                    if videos:
                        video_to_watch = random.choice(videos)
                        print(f"[{client_id}] 'Watching' {video_to_watch['name']}")
                        # We don't need to download the whole thing, just make the request
                        requests.get(f"{BASE_URL}/video/{video_to_watch['name']}", timeout=5, stream=True)
                except requests.RequestException as e:
                    print(f"[{client_id}] Could not 'watch' video: {e}")


        except requests.RequestException as e:
            print(f"[{client_id}] Failed to send heartbeat: {e}")
        
        time.sleep(5)

if __name__ == "__main__":
    main()
