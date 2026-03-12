<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER TYPECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00294">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
	  	[ISM-ID-00294][Error] All @ism:resourceElement attributes must be of type Boolean. 
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all elements which contain an @ism:resourceElement attribute, this rule ensures that the resourceElement value matches the pattern
		defined for type Boolean. 
		
		Note: this rule is not able to be failed. If the resourceElement does
		not confirm to type Boolean, schematron fails when defining global
		variables before any rules are fired. 
	</sch:p>
	  <sch:rule id="ISM-ID-00294-R1" context="*[@ism:resourceElement]">
		    <sch:assert test="util:meetsType(@ism:resourceElement, $BooleanPattern)" flag="error" role="error">
		    	[ISM-ID-00294][Error] All @ism:resourceElement attributes must be of type Boolean. 
		</sch:assert>
	  </sch:rule>
</sch:pattern>