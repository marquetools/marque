<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00174">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00174][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:atomicEnergyMarkings contains the name token [RD], [FRD], or [TFNI], 
        then attribute @ism:classification must have a value of [TS], [S], or [C].
        
        Human Readable: USA documents with RD, FRD, or TFNI data must be marked CONFIDENTIAL,
        SECRET, or TOP SECRET.
    </sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		If the document is an ISM_USGOV_RESOURCE, for each element which has 
		attribute @ism:atomicEnergyMarkings specified with a value containing 
		the token [RD], [FRD], or [TFNI], this rule ensures that the attribute 
		@ism:classification has a value of [TS], [S], or [C].
	</sch:p>
	  <sch:rule id="ISM-ID-00174-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD', 'FRD', 'TFNI'))]">
		    <sch:assert test="@ism:classification = ('TS','S','C')" flag="error" role="error">
		        [ISM-ID-00174][Error] If ISM_USGOV_RESOURCE and attribute 
		        @ism:atomicEnergyMarkings contains the name token [RD], [FRD], or [TFNI], 
		        then attribute @ism:classification must have a value of [TS], [S], or [C].
		        
		        Human Readable: USA documents with RD, FRD, or TFNI data must be marked CONFIDENTIAL,
		        SECRET, or TOP SECRET.
		</sch:assert>
	  </sch:rule>
</sch:pattern>