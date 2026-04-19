<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="PORTION"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00185">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00185][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:atomicEnergyMarkings contains the name token [RD-CNWDI],
        then it must also contain the name token [RD].
    </sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		If the document is an ISM_USGOV_RESOURCE, for each element which has 
		attribute @ism:atomicEnergyMarkings specified with a value containing 
		the token [RD-CNWDI], this rule ensures that attribute 
		@ism:atomicEnergyMarkings also contains the token [RD].
	</sch:p>
	  <sch:rule id="ISM-ID-00185-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:atomicEnergyMarkings, ('RD-CNWDI'))]">
		    <sch:assert test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('RD'))" flag="error" role="error">
		    	[ISM-ID-00185][Error] If ISM_USGOV_RESOURCE and attribute 
		    	@ism:atomicEnergyMarkings contains the name token [RD-CNWDI],
		    	then it must also contain the name token [RD].
		</sch:assert>
	  </sch:rule>
</sch:pattern>