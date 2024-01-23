import sys
import struct
from PIL import Image
import os

def get_pixel_value(file, x, y, width, bytes_per_pixel):
    # Calculate the position in the file
    position = (y * width + x) * bytes_per_pixel
    file.seek(position)
    # Read the 16 bytes (4 channels x 4 bytes each) for the pixel
    pixel_data = file.read(bytes_per_pixel)
    if not pixel_data:
        raise ValueError("Pixel position is out of bounds")
    # Unpack the data assuming it's 4 floats (32 bits each)
    return struct.unpack('ffff', pixel_data)

def process_image(bin_file_path, width, height, bytes_per_pixel):
    image = Image.new("RGBA", (width, height))
    with open(bin_file_path, 'rb') as file:
        for y in range(height):
            for x in range(width):
                pixel_data = file.read(bytes_per_pixel)
                if not pixel_data:
                    raise ValueError("File ended unexpectedly")
                rgba = struct.unpack('ffff', pixel_data)
                max_color = max(rgba[:3])
                if max_color == 0:  # Avoid division by zero
                    normalized_rgba = (0, 0, 0, 1)
                else:
                    normalized_rgba = tuple(channel / max_color for channel in rgba[:3]) + (1,)
                image.putpixel((x, y), tuple(int(255 * c) for c in normalized_rgba))
    return image

if __name__ == "__main__":
    if len(sys.argv) != 4:
        print("Usage: python script.py <bin_file_path> <x> <y>")
        sys.exit(1)

    bin_file_path = sys.argv[1]
    x = int(sys.argv[2])
    y = int(sys.argv[3])

    try:
        # Process the entire image
        image = process_image(bin_file_path, 512, 512, 16)
        output_file_name = os.path.splitext(bin_file_path)[0] + ".png"
        image.save(output_file_name)
        print(f"Image saved as '{output_file_name}'")

        # Output specific pixel value
        with open(bin_file_path, 'rb') as file:
            pixel_value = get_pixel_value(file, x, y, 512, 16)
            print(f"Pixel value at ({x}, {y}): {pixel_value}")

    except Exception as e:
        print(f"Error: {e}")
