<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00299">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00299][Error] If an element contains the attribute @ism:declassException with a value of [AEA], 
        it must also contain the attribute @ism:atomicEnergyMarkings.
    </sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
		If an element contains an @ism:declassException attribute with a value containing
		[AEA], this rule checks to make sure that element also has an @ism:atomicEnergyMarkings
		attribute.
	</sch:p>
	  <sch:rule id="ISM-ID-00299-R1" context="*[util:containsAnyTokenMatching(@ism:declassException, ('AEA'))]">
		    <sch:assert test="@ism:atomicEnergyMarkings" flag="error" role="error">
		    	[ISM-ID-00299][Error] If an element contains the attribute @ism:declassException with a value of [AEA], 
		    	it must also contain the attribute @ism:atomicEnergyMarkings.
		</sch:assert>
	  </sch:rule>
</sch:pattern>