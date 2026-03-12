<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00184">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00184][Error] If ISM_USGOV_RESOURCE and attribute 
        @ism:atomicEnergyMarkings contains a name token starting with [FRD-SG],
        then it must also contain the name token [FRD].
    </sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		If the document is an ISM_USGOV_RESOURCE, for each element which has 
		attribute @ism:atomicEnergyMarkings specified with a value containing a 
		token starting with [FRD-SG], this rule ensures that attribute 
		@ism:atomicEnergyMarkings also contains the token [FRD].
	</sch:p>
	  <sch:rule id="ISM-ID-00184-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:atomicEnergyMarkings, ('^FRD-SG'))]">
		    <sch:assert test="util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('FRD'))" flag="error" role="error">
		    	[ISM-ID-00184][Error] If ISM_USGOV_RESOURCE and attribute 
		    	@ism:atomicEnergyMarkings contains a name token starting with [FRD-SG],
		    	then it must also contain the name token [FRD].
		</sch:assert>
	  </sch:rule>
</sch:pattern>