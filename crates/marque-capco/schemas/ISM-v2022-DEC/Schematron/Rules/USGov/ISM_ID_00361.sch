<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00361">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00361][Error] All @ism:hasApproximateMarkings attributes must be of type Boolean. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:hasApproximateMarkings attribute, this rule ensures that the 
	  	hasApproximateMarkings value matches the pattern defined for type Boolean. 
	</sch:p>
	<sch:rule id="ISM-ID-00361-R1" context="*[@ism:hasApproximateMarkings]">
		<sch:assert test="util:meetsType(@ism:hasApproximateMarkings, $BooleanPattern)" flag="error" role="error">
		    	[ISM-ID-00361][Error] All @ism:hasApproximateMarkings attributes values must be of type Boolean. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>