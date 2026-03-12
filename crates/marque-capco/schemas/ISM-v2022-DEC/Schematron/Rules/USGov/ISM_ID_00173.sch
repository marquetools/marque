<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00173">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00173][Error] If ISM_USGOV_RESOURCE and attribute
        @ism:atomicEnergyMarkings contains a name token starting with [RD-SG] or [FRD-SG], then attribute
        @ism:classification must have a value of [S] or [TS]. 
        
        Human Readable: Portions in a USA document that contain RD or FRD SIGMA data must be marked SECRET or TOP SECRET. 
    </sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	      If the document is an ISM_USGOV_RESOURCE, for each element which has
        attribute @ism:atomicEnergyMarkings specified with a value containing a token starting with
        [RD-SG] or [FRD-SG], this rule ensures that the attribute @ism:classification has a value of [S] or [TS]. 
	  </sch:p>
    <sch:rule id="ISM-ID-00173-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:atomicEnergyMarkings, ('^RD-SG', '^FRD-SG'))]">
		    <sch:assert test="@ism:classification = ('S','TS')" flag="error" role="error">
		        [ISM-ID-00173][Error] If ISM_USGOV_RESOURCE and attribute
		        @ism:atomicEnergyMarkings contains a name token starting with [RD-SG] or [FRD-SG], then attribute
		        @ism:classification must have a value of [S] or [TS]. 
		        
		        Human Readable: Portions in a USA document that contain RD or FRD SIGMA data must be marked SECRET or TOP SECRET. 
		    </sch:assert>
	  </sch:rule>
</sch:pattern>