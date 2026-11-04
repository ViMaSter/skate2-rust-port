// Command skate2-ad-server is a local stand-in for the web server the original game fetched its
// in-game billboard ads from (`Sk8::Net::Massive`). It serves PNG files converted to JPEG,
// or generated test ads in glaring colours. The protocol is the PC port's own
// (docs/online/remote-ads.md): the original's is not recovered.
//
//	GET /skate2-ads/v1/ping                 "skate2-ads 1"
//	GET /skate2-ads/v1/manifest.txt         "skate2-ads 1", then "ad <id> <width> <height> <path>"
//	GET /skate2-ads/v1/image/<id>.jpg       the ad's JPEG
package main

import (
	"bytes"
	"flag"
	"fmt"
	"image"
	"image/color"
	"image/jpeg"
	"log"
	"net/http"
	"sort"
	"strings"
)

const apiPrefix = "/skate2-ads/v1"

type ad struct {
	id            string
	width, height int
	jpeg          []byte
}

// size is one billboard texture size of the world. 256x128, 128x256 and 512x256 are the
// horizontal, vertical and wide billboards; the rest are the small signs.
type size struct{ width, height int }

var sizes = []size{{256, 128}, {128, 256}, {512, 256}, {128, 128}, {64, 64}, {64, 32}}

type palette struct {
	name       string
	back, line color.RGBA
}

// Every ad of one size has another colour, so the world's several placeholders of a size
// (vertical_01, _02, _03) tell apart.
var palettes = []palette{
	{"magenta", color.RGBA{255, 0, 220, 255}, color.RGBA{255, 255, 0, 255}},
	{"lime", color.RGBA{60, 255, 0, 255}, color.RGBA{255, 0, 120, 255}},
	{"cyan", color.RGBA{0, 235, 255, 255}, color.RGBA{255, 90, 0, 255}},
	{"orange", color.RGBA{255, 140, 0, 255}, color.RGBA{0, 60, 255, 255}},
}

func main() {
	address := flag.String("addr", "127.0.0.1:8745", "listen address; the game looks for 127.0.0.1:8745")
	directory := flag.String("ads", "", "folder of PNG ads, in filename order (default: generated test ads)")
	examples := flag.String("write-examples", "", "write numbered example PNGs to this folder, without overwriting, then exit")
	flag.Parse()
	if *examples != "" {
		if err := writeExamples(*examples); err != nil {
			log.Fatal(err)
		}
		return
	}
	var ads []ad
	var err error
	if *directory == "" {
		ads, err = generate()
	} else {
		ads, err = loadDirectory(*directory)
	}
	if err != nil {
		log.Fatal(err)
	}
	log.Printf("serving %d ads on http://%s%s/", len(ads), *address, apiPrefix)
	log.Fatal(http.ListenAndServe(*address, handler(ads)))
}

func generate() ([]ad, error) {
	var ads []ad
	for _, s := range sizes {
		for index, p := range palettes {
			var encoded bytes.Buffer
			if err := jpeg.Encode(&encoded, render(s, index, p), &jpeg.Options{Quality: 95}); err != nil {
				return nil, err
			}
			ads = append(ads, ad{
				id:     fmt.Sprintf("%s-%dx%d", p.name, s.width, s.height),
				width:  s.width,
				height: s.height,
				jpeg:   encoded.Bytes(),
			})
		}
	}
	// The manifest's order is what the game picks from, so it is fixed.
	sort.SliceStable(ads, func(a, b int) bool { return ads[a].id < ads[b].id })
	return ads, nil
}

func handler(ads []ad) http.Handler {
	byID := make(map[string]ad, len(ads))
	for _, a := range ads {
		byID[a.id] = a
	}
	mux := http.NewServeMux()
	mux.HandleFunc("GET "+apiPrefix+"/ping", func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Type", "text/plain")
		fmt.Fprintln(w, "skate2-ads 1")
	})
	mux.HandleFunc("GET "+apiPrefix+"/manifest.txt", func(w http.ResponseWriter, _ *http.Request) {
		var text strings.Builder
		text.WriteString("skate2-ads 1\n")
		for _, a := range ads {
			fmt.Fprintf(&text, "ad %s %d %d %s/image/%s.jpg\n", a.id, a.width, a.height, apiPrefix, a.id)
		}
		w.Header().Set("Content-Type", "text/plain")
		fmt.Fprint(w, text.String())
	})
	mux.HandleFunc("GET "+apiPrefix+"/image/{name}", func(w http.ResponseWriter, r *http.Request) {
		name, jpegPath := strings.CutSuffix(r.PathValue("name"), ".jpg")
		a, ok := byID[name]
		if !jpegPath || !ok {
			http.NotFound(w, r)
			return
		}
		w.Header().Set("Content-Type", "image/jpeg")
		w.Write(a.jpeg)
	})
	return logged(mux)
}

func logged(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		log.Printf("%s %s", r.RemoteAddr, r.URL.Path)
		next.ServeHTTP(w, r)
	})
}

var white = color.RGBA{255, 255, 255, 255}

// render draws one ad: a flat bright colour, diagonal stripes, a border in the contrasting
// colour, a white triangle in the top left corner (a billboard showing it elsewhere is mapped
// mirrored or turned) and the ad's number and size in blocky digits.
func render(s size, index int, p palette) *image.RGBA {
	img := image.NewRGBA(image.Rect(0, 0, s.width, s.height))
	unit := max(1, min(s.width, s.height)/32)
	border := 3 * unit
	period := 16 * unit
	for y := 0; y < s.height; y++ {
		for x := 0; x < s.width; x++ {
			c := p.back
			switch {
			case x < border || y < border || x >= s.width-border || y >= s.height-border:
				c = p.line
			case (x-border)+(y-border) < 10*unit:
				c = white
			case (x+y)%period < period/4:
				c = p.line
			}
			img.SetRGBA(x, y, c)
		}
	}
	text(img, fmt.Sprintf("AD %d", index+1), s, 0.26, 0.18)
	text(img, fmt.Sprintf("%dX%d", s.width, s.height), s, 0.2, 0.6)
	return img
}

// text centres a line of blocky text on the image, `height` of the image tall with its top at
// `top` of the image's height: black on a white plate.
func text(img *image.RGBA, line string, s size, height, top float64) {
	cell := max(1, int(float64(s.height)*height)/7)
	for cell > 1 && cell*6*len(line) > s.width*9/10 {
		cell--
	}
	width := cell * 6 * len(line)
	left := (s.width - width) / 2
	y0 := int(float64(s.height) * top)
	fill(img, image.Rect(left-cell, y0-cell, left+width, y0+8*cell), white)
	for n, r := range line {
		for row, bits := range glyphs[r] {
			for col, bit := range bits {
				if bit == '#' {
					x, y := left+(n*6+col)*cell, y0+row*cell
					fill(img, image.Rect(x, y, x+cell, y+cell), color.RGBA{0, 0, 0, 255})
				}
			}
		}
	}
}

func fill(img *image.RGBA, area image.Rectangle, c color.RGBA) {
	area = area.Intersect(img.Bounds())
	for y := area.Min.Y; y < area.Max.Y; y++ {
		for x := area.Min.X; x < area.Max.X; x++ {
			img.SetRGBA(x, y, c)
		}
	}
}

var glyphs = map[rune][7]string{
	'A': {" ### ", "#   #", "#   #", "#####", "#   #", "#   #", "#   #"},
	'D': {"#### ", "#   #", "#   #", "#   #", "#   #", "#   #", "#### "},
	'X': {"#   #", "#   #", " # # ", "  #  ", " # # ", "#   #", "#   #"},
	' ': {"     ", "     ", "     ", "     ", "     ", "     ", "     "},
	'0': {" ### ", "#   #", "#  ##", "# # #", "##  #", "#   #", " ### "},
	'1': {"  #  ", " ##  ", "  #  ", "  #  ", "  #  ", "  #  ", " ### "},
	'2': {" ### ", "#   #", "    #", "   # ", "  #  ", " #   ", "#####"},
	'3': {" ### ", "#   #", "    #", "  ## ", "    #", "#   #", " ### "},
	'4': {"   # ", "  ## ", " # # ", "#  # ", "#####", "   # ", "   # "},
	'5': {"#####", "#    ", "#### ", "    #", "    #", "#   #", " ### "},
	'6': {" ### ", "#    ", "#    ", "#### ", "#   #", "#   #", " ### "},
	'7': {"#####", "    #", "   # ", "  #  ", " #   ", " #   ", " #   "},
	'8': {" ### ", "#   #", "#   #", " ### ", "#   #", "#   #", " ### "},
	'9': {" ### ", "#   #", "#   #", " ####", "    #", "    #", " ### "},
}
