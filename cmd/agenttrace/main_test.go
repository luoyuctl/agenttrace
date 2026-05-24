package main

import "testing"

func TestHasPostPricingAction(t *testing.T) {
	tests := []struct {
		name       string
		path       string
		listModels bool
		testMatch  bool
		doctor     bool
		latest     bool
		compare    bool
		overview   bool
		waste      bool
		search     string
		want       bool
	}{
		{name: "update pricing alone exits", want: false},
		{name: "list models continues", listModels: true, want: true},
		{name: "test match continues", testMatch: true, want: true},
		{name: "doctor continues", doctor: true, want: true},
		{name: "overview continues", overview: true, want: true},
		{name: "latest continues", latest: true, want: true},
		{name: "compare continues", compare: true, want: true},
		{name: "waste continues", waste: true, want: true},
		{name: "search continues", search: "billing", want: true},
		{name: "blank search exits", search: "   ", want: false},
		{name: "path continues", path: "session.jsonl", want: true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := hasPostPricingAction(tt.path, tt.listModels, tt.testMatch, tt.doctor, tt.latest, tt.compare, tt.overview, tt.waste, tt.search)
			if got != tt.want {
				t.Fatalf("hasPostPricingAction() = %v, want %v", got, tt.want)
			}
		})
	}
}
