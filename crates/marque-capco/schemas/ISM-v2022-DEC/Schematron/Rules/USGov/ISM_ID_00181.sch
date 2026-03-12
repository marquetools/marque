<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00181">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00181][Error] If ISM_USGOV_RESOURCE and element's classification does not have a value of "U" 
        then attribute @ism:atomicEnergyMarkings must not contain the name token [UCNI] or [DCNI].
        
        Human Readable: UCNI and DCNI may only be used on UNCLASSIFIED portions.
    </sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		If the document is an ISM_USGOV_RESOURCE, for each element which has 
		attribute @ism:atomicEnergyMarkings specified and has attribute @ism:classification specified with a value other than [U], 
		this rule ensures that attribute @ism:atomicEnergyMarkings does not contain the token [UCNI] or [DNCI].
	</sch:p>
	  <sch:rule id="ISM-ID-00181-R1" context="*[$ISM_USGOV_RESOURCE and @ism:atomicEnergyMarkings and not(@ism:classification='U')]">
		    <sch:assert test="not(util:containsAnyOfTheTokens(@ism:atomicEnergyMarkings, ('UCNI', 'DCNI')))" flag="error" role="error">
		        [ISM-ID-00181][Error] If ISM_USGOV_RESOURCE and element's classification does not have a value of "U" 
		        then attribute @ism:atomicEnergyMarkings must not contain the name token [UCNI] or [DCNI].
		        
		        Human Readable: UCNI and DCNI may only be used on UNCLASSIFIED portions.
		</sch:assert>
	  </sch:rule>
</sch:pattern>