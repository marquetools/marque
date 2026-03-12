<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00527">
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
		[ISM-ID-00527][Warning] All resource elements that contain a DoD @ism:SARIdentifier attribute SHOULD contain attribute
		@ism:declassException.
	</sch:p>
	  <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
	  	For all resource elements which contain a DoD @ism:SARIdentifier attribute, this rule raises a WARNING flag that the 
	  	resource element SHOULD also have an @ism:declassException attribute.  DoD SARs are identified by an @ism:SARIdentifier that
	  	starts with 'SAR-DOD:'.
	</sch:p>
	<sch:rule id="ISM-ID-00527-R1" context="*[@ism:resourceElement='true' and @ism:SARIdentifier]">
		<sch:assert test="if ((some $token in tokenize(normalize-space(string(./@ism:SARIdentifier)),' ') 
			satisfies starts-with($token,'SAR-DOD:')) and @ism:declassException) then true()
			else if ((some $token in tokenize(normalize-space(string(./@ism:SARIdentifier)),' ') 
			satisfies starts-with($token,'SAR-DOD:')) and not(@ism:declassException)) then false()
			else true()" 
			flag="warning" 
			role="warning">
		    	[ISM-ID-00527][Warning] All resource elements that contain a DOD @ism:SARIdentifier attribute SHOULD contain attribute
		    	@ism:declassException. Per the OSD Declassification Guide, there is an ISCAP Files Series Exemption (FSE) on 
		    	records within DoD Special Access Programs (SAPs) files. This Exemption functions as a 25X, and therefore the records 
		    	in these files are exempted from automatic declassification for 50 years. This document does not apply any declassification 
		    	exemption; recommend verifying that this is correct.
		</sch:assert>
	  </sch:rule>
</sch:pattern>